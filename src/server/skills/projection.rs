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
//! The exact bytes this module produces are pinned by a golden file —
//! `tests/golden/workflow_skill_projection.md`, compared byte-for-byte by
//! `golden_render_is_byte_equal` in `tests/skills_integration.rs` — so that no
//! change is accidental. They are explicitly **not** part of the crate's semver
//! contract: the render may change on any minor bump (D-14). It changes with a
//! CHANGELOG entry every time, because the bytes become the `sha256` digest
//! published in the skill's `skills/list` entry, and a consumer that pinned that
//! digest must re-pin. A digest mismatch is a fatal pre-loop revocation for such
//! a consumer, not a warning — so a silent render change is a supply-chain
//! event, not a cosmetic one.
//!
//! When that golden goes red it has exactly two causes needing opposite fixes:
//! the render changed on purpose (update the golden AND write the CHANGELOG
//! entry) or the render regressed (fix the renderer). Re-recording the golden
//! from current output resolves the symptom without deciding which. The
//! comparison's failure message says so at the point of failure.
//!
//! # Two workflow accessors are excluded from the render on purpose
//!
//! [`SequentialWorkflow::has_task_support`] and [`WorkflowStep::is_retryable`]
//! are server-execution mechanics with no manual analogue: a human or a client
//! LLM following the rendered procedure by hand neither schedules a task nor
//! retries a step the way the server's own executor does. Rendering them would
//! put facts in the body that the reader cannot act on. They are excluded
//! deliberately (D-11), which is what makes SC-3's "every workflow fact renders"
//! a DEFINED universe rather than an open-ended one — the exclusions are named
//! here, and everything else renders.
//!
//! `sc3_excluded_execution_mechanics_change_no_byte` pins their absence by BYTE
//! EQUALITY: it renders the same fixture with both accessors at their defaults
//! and with both non-default, and compares. An accessor-name-absence check is
//! kept alongside it as a readability guard only — a name check passes the
//! moment a future render spells the fact differently.
//!
//! [`SequentialWorkflow::has_task_support`]: crate::server::workflow::SequentialWorkflow::has_task_support
//! [`WorkflowStep::is_retryable`]: crate::server::workflow::WorkflowStep::is_retryable
//!
//! # No tool description and no input schema reaches the body (D-12)
//!
//! Steps render their tool NAME and nothing else. The client reading this skill
//! is already connected to the server that serves it and has `tools/list` one
//! call away, so a tool's description and input schema are always available and
//! always current. A copy digested into this body could only go stale, and
//! staleness in a digested document is worse than absence: it looks
//! authoritative.
//!
//! # Frontmatter carries `name` and `description` only (D-13)
//!
//! Hosts verify frontmatter field-by-field against the fetched file, so every
//! additional key is additional conformance surface that can disagree. Two keys
//! is the agentskills-legal minimum and therefore the smallest risk.
//! `sc3_frontmatter_carries_exactly_two_keys` pins the count.
//!
//! # Frontmatter is written as encoded YAML scalars, never by concatenation
//!
//! Both frontmatter values go through the module-private `yaml_double_quoted`,
//! **unconditionally**, for two independent reasons:
//!
//! 1. A slug drawn from `[a-z0-9-]` can still be a YAML type-alike — `true`,
//!    `123`, `null`. Unquoted, such a value parses as a bool or an integer, and
//!    the registry's `validate_names` guard reaches for `as_str()`, gets `None`,
//!    and SKIPS the skill rather than enforcing the name-identity check. The
//!    check silently not running is worse than it failing.
//! 2. Conditional quoting would make the digested bytes depend on a predicate
//!    over author text, so an innocuous description edit could move the digest
//!    through a branch nobody was thinking about. Unconditional encoding removes
//!    the branch.
//!
//! A workflow description is arbitrary author text, and pushing it raw after
//! `description: ` lets an ordinary string like `Refund an order: fast path`
//! break the YAML parse — which the registry DOWNGRADES to a diagnostic rather
//! than an error, silently skipping the name-identity check instead of
//! enforcing it. Encoding both values unconditionally removes the whole class.
//!
//! The tripwire is `prop_frontmatter_roundtrips`, whose ORACLE is the module's
//! own `parse_frontmatter_value`: the encoder is hand-written, the decoder is
//! library-verified, so round-tripping arbitrary text through both is a real
//! check rather than a mirror of the encoder's own assumptions.
//!
//! # Description legality: substituted or rejected, never length-bounded
//!
//! `SequentialWorkflow::new` validates nothing, so an empty description is legal
//! input while `description:` with nothing after it renders a YAML null and
//! violates agentskills' non-empty rule. The infallible path substitutes a
//! deterministic legal string; [`SkillProjection::build`] returns `Err` instead,
//! pushing a strict caller to write one.
//!
//! **No length bound is enforced, because the current SEP-2640 / agentskills
//! material states none.** It specifies the digest format, the per-skill file
//! and byte limits and the name-identity rule, and says nothing about
//! description length. The omission here is deliberate, not an oversight — do
//! not invent a limit; the registry's existing over-limit warning already covers
//! a pathologically large body.
//!
//! # Every `#[non_exhaustive]` catch-all arm emits a constant literal
//!
//! [`PromptContent`] and [`DataSource`] are `#[non_exhaustive]`, so a variant
//! added upstream lands on this module's catch-all arms. Those arms emit ONE
//! STABLE LITERAL each — never a `{:?}` of the value. A `Debug` fallback would
//! let an upstream variant addition silently move the rendered bytes and
//! invalidate every published digest, which is precisely the class D-14 exists
//! to make loud. The arms carry `#[allow(unreachable_patterns)]` because within
//! this crate the matches are exhaustive today; the allow is what buys the
//! future-variant guard.
//!
//! # SC-6 gate warnings have exactly ONE delivery channel
//!
//! The gate warning — guidance attached to a step whose tool is annotated
//! side-effecting — is delivered through [`SkillProjection::build`]'s structured
//! return, and only when [`SkillProjection::with_tools`] supplied annotations.
//! `SequentialWorkflow::as_skill` receives only a `&SequentialWorkflow`, and
//! [`ToolAnnotations`] live solely on [`crate::types::ToolInfo::annotations`],
//! which nothing reachable from a workflow carries — so `as_skill()` cannot
//! COMPUTE the warning; it is not that it declines to log it. Do not describe
//! SC-6 as having two channels. (The builder additionally logs each warning it
//! returns on `mcp.skills`, but that is the same finding on the same path, not a
//! second way to obtain it.)
//!
//! # `SequentialWorkflow::instruction()` becomes observable here, and only here
//!
//! `instruction()` is a shipped public builder method whose value every served
//! surface currently drops. The `## Context` section this module renders is the
//! FIRST place that text becomes observable to any consumer. That underlying
//! defect is deliberately out of scope here — this note exists so a maintainer
//! who notices the asymmetry knows it was seen, not missed.
//!
//! # Opting a server into the projected prepend
//!
//! [`WorkflowPromptHandler::with_projected_skill_prepend`] makes the projected
//! body prompt message `[0]`, and
//! [`ServerCoreBuilder::with_workflow_skill_prepend`] /
//! [`ServerBuilder::with_workflow_skill_prepend`] reach that opt-in from
//! `prompt_workflow` on either builder. All three default to OFF, so a
//! flag-off transcript is byte-identical to one from before they existed.
//!
//! The builder setters read their flag at `prompt_workflow` time, so each
//! applies to workflows registered AFTER the call. Their existence is what makes
//! the anti-drift claim hold **per server** rather than merely per workflow
//! value: without them the opt-in was reachable only by hand-constructing a
//! `WorkflowPromptHandler`, and a renderer whose consumer is unreachable leaves
//! the claim theoretical.
//!
//! [`PromptContent`]: crate::server::workflow::PromptContent
//! [`DataSource`]: crate::server::workflow::DataSource
//! [`ToolAnnotations`]: crate::types::ToolAnnotations
//! [`WorkflowPromptHandler::with_projected_skill_prepend`]: crate::server::workflow::WorkflowPromptHandler::with_projected_skill_prepend
//! [`ServerCoreBuilder::with_workflow_skill_prepend`]: crate::server::builder::ServerCoreBuilder::with_workflow_skill_prepend
//! [`ServerBuilder::with_workflow_skill_prepend`]: crate::server::ServerBuilder::with_workflow_skill_prepend

use crate::error::{Error, Result};
use crate::server::skills::Skill;
use crate::server::workflow::{DataSource, PromptContent, SequentialWorkflow, WorkflowStep};
use crate::types::{PromptArgumentType, ToolAnnotations};
use std::collections::{BTreeMap, HashMap};

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

/// The log-forging classification and the lossy substitution both live in
/// [`crate::shared::log_sanitize`], shared with `server::core`'s `skills/get`
/// URI echo — the same mitigation, previously written twice and already drifted
/// (that copy tested `is_control()` alone, reaching neither `U+2028` nor
/// `U+2029`).
///
/// [`yaml_double_quoted`] below shares only the CLASSIFICATION and deliberately
/// not the substitution: a YAML scalar is re-parsable text with an escape
/// vocabulary, so it escapes where a log field replaces (WR-06).
use crate::shared::log_sanitize::sanitize_for_log;

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
            // YAML 1.1 — which is what `serde_yaml 0.9` speaks, through
            // `unsafe-libyaml`'s `IS_BREAK_AT!` — counts FIVE characters as
            // line breaks: `\r`, `\n`, `U+0085`, `U+2028` and `U+2029`. Only
            // the first three are Unicode `Cc`, so the `is_control()` arm below
            // never reaches the last two (see `is_unicode_line_separator`).
            // Emitted raw they either fold the blanks adjacent to them out of
            // the value, or — when the text after them begins `---`/`...` —
            // fail the parse outright, which the registry DOWNGRADES to a
            // diagnostic and then silently drops the skill from `skills/list`
            // while `into_handler()` still returns `Ok` (CR-01). That is the
            // exact class the module header claims to have removed.
            //
            // `\uNNNN` rather than YAML 1.1's dedicated `\L`/`\P`: MEASURED
            // against this workspace's resolved `serde_yaml 0.9.34+deprecated`
            // / `unsafe-libyaml 0.2.11` pair, all four spellings round-trip, so
            // the choice is about the OTHER readers of these bytes — a SKILL.md
            // frontmatter block is a published artifact (D-14) that third-party
            // hosts parse with their own YAML implementations, and `\uNNNN` is
            // accepted by every mainstream one while `\L`/`\P` are rare.
            //
            // NOT `\xNN`, which is the trap this fix has to step around:
            // libyaml's `\x` consumes EXACTLY two hex digits, so `\x2028`
            // decodes as `U+0020` followed by the LITERAL text `28` (measured).
            // The `\xNN` arm below is sound only because `Cc`'s maximum scalar
            // is `U+009F` and two digits therefore always suffice there.
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            // The two BMP NONCHARACTERS, for the SAME reason and by the same
            // spelling. YAML 1.1's printable set — which `unsafe-libyaml`'s
            // reader enforces — admits `U+E000..=U+FFFD` and stops there, so
            // `U+FFFE` and `U+FFFF` are rejected as unacceptable characters.
            // Neither is Unicode `Cc`, so the `is_control()` arm below does not
            // reach them, and neither is a line separator, so the arms above do
            // not either: emitted raw they make the rendered frontmatter block
            // FAIL to parse, which the registry downgrades to a diagnostic and
            // then silently drops the skill from `skills/list` while
            // `into_handler()` still returns `Ok` — the CR-01 class again, in
            // the one gap the original escape table left. They are the ONLY
            // gap: every other non-printable BMP scalar is `Cc`, surrogates
            // cannot exist in a `char`, and the supplementary noncharacters
            // (`U+1FFFE` and friends) are inside libyaml's 4-byte accept range.
            '\u{fffe}' | '\u{ffff}' => out.push_str(&format!("\\u{:04x}", c as u32)),
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
    if description_is_absent(wf) {
        fallback_description(slug)
    } else {
        wf.description().to_string()
    }
}

/// The ONE spelling of "this workflow has no usable description".
///
/// `.trim()` is load-bearing and used to be missing here. `SkillProjection::build`
/// has always rejected on `description().trim().is_empty()`, while this path
/// substituted only on the UNTRIMMED `is_empty()` — so a description of `"   "`
/// was rejected by the fallible entry point and passed straight through by the
/// infallible one, rendering `description: "   "` into a frontmatter block whose
/// bytes are a published digest (D-14) and emitting no
/// [`ProjectionNotice::EmptyDescription`], so the operator was never told. The
/// two entry points are documented to differ in DISPOSITION over the same
/// condition; sharing this predicate is what makes that true.
fn description_is_absent(wf: &SequentialWorkflow) -> bool {
    wf.description().trim().is_empty()
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

    if description_is_absent(wf) {
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

// ── The fallible builder path (D-01 / D-02) ───────────────────────────

/// Why a [`ProjectionWarning`] was emitted.
///
/// `#[non_exhaustive]`, so a later phase can add a kind without a breaking
/// change; match on it with a `_` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProjectionWarningKind {
    /// SC-6 / D-09: a step carries `with_guidance` prose AND its tool is
    /// annotated side-effecting (`read_only_hint == Some(false)` or
    /// `destructive_hint == Some(true)`).
    ///
    /// Server-side workflow execution runs every deterministic step regardless
    /// of the guidance, so guidance attached to a side-effecting step is a
    /// post-hoc judgment the executing surface will ignore. The trigger is
    /// purely structural — the prose is never analysed (D-09), so it cannot be
    /// paraphrased around.
    GuidanceOnSideEffectingStep,
    /// D-08: the SC-6 check could not be performed for a guidance-bearing step,
    /// because its tool carries no annotations or is absent from the supplied
    /// tool map.
    ///
    /// This is deliberately DISTINCT from
    /// [`Self::GuidanceOnSideEffectingStep`]. MCP's literal annotation defaults
    /// (an absent `read_only_hint` meaning not-read-only) are NOT followed:
    /// that would fire on essentially every existing workflow, and a warning
    /// that fires everywhere is a warning that gets muted. Reporting "could not
    /// check" honestly is the alternative D-08 chose.
    GateCheckUnverifiable,
    /// D-15: the workflow name normalized to nothing legal and a deterministic
    /// `workflow-{8 hex}` slug was substituted.
    ///
    /// [`SkillProjection::build`] REJECTS that input rather than substituting
    /// (see its `# Errors`), so the builder path does not emit this kind today.
    /// It is declared so the warning vocabulary covers the conditions the
    /// infallible [`SequentialWorkflow::as_skill`] resolves, and so a future
    /// lenient mode is additive rather than breaking.
    ///
    /// [`SequentialWorkflow::as_skill`]: crate::server::workflow::SequentialWorkflow::as_skill
    SlugFallback,
}

/// One condition the projection found worth reporting to the server author.
///
/// # Why this type is public where `SkillDiagnostic` is not
///
/// This module's sibling `SkillDiagnostic` (`src/server/skills/mod.rs`) is
/// crate-private ON PURPOSE, as its own rustdoc records: it exists to be
/// logged, not to be matched on. `ProjectionWarning` deliberately departs from
/// that precedent because D-10 requires [`SkillProjection::build`] to RETURN
/// its warnings — so that tests can assert on data instead of installing a
/// `tracing` subscriber, and so that a caller can act on them (fail a build,
/// annotate a review, surface them in a CLI). Do not "fix" the asymmetry by
/// making this crate-private: that would break `build()`'s signature and remove
/// the only channel SC-6 warnings have.
///
/// It is a struct rather than an enum so that added fields stay additive.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "skills")] {
/// use pmcp::server::skills::{ProjectionWarning, SkillProjection};
/// use pmcp::server::workflow::SequentialWorkflow;
///
/// let workflow = SequentialWorkflow::new("refund_flow", "Process a refund");
/// let warnings: Vec<ProjectionWarning> = SkillProjection::new(&workflow)
///     .build()
///     .expect("a legal name and a non-empty description")
///     .warnings;
///
/// // No tool map was supplied, so the SC-6 gate check did not run (D-07).
/// assert!(warnings.is_empty());
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ProjectionWarning {
    kind: ProjectionWarningKind,
    step: Option<String>,
    tool: Option<String>,
    message: String,
}

impl ProjectionWarning {
    /// Construct a warning. Module-private: warnings are produced by the
    /// projection, never by a caller.
    fn new(
        kind: ProjectionWarningKind,
        step: Option<String>,
        tool: Option<String>,
        message: String,
    ) -> Self {
        Self {
            kind,
            step,
            tool,
            message,
        }
    }
    /// Which condition this warning reports.
    pub fn kind(&self) -> ProjectionWarningKind {
        self.kind
    }

    /// The workflow step this warning is about, when it is about one.
    pub fn step(&self) -> Option<&str> {
        self.step.as_deref()
    }

    /// The tool this warning is about, when it is about one.
    pub fn tool(&self) -> Option<&str> {
        self.tool.as_deref()
    }

    /// The human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Is this tool ANNOTATED as side-effecting?
///
/// True only for an explicit `read_only_hint == Some(false)` or
/// `destructive_hint == Some(true)`. MCP's own literal defaults — where an
/// ABSENT `read_only_hint` means not-read-only — are deliberately NOT followed
/// (D-08): under them essentially every existing workflow would trip the gate,
/// and a warning that fires everywhere is a warning that gets muted. The
/// missing-annotation case is reported honestly as
/// [`ProjectionWarningKind::GateCheckUnverifiable`] instead of being guessed at.
fn is_annotated_side_effecting(annotations: &ToolAnnotations) -> bool {
    annotations.read_only_hint == Some(false) || annotations.destructive_hint == Some(true)
}

/// Classify ONE guidance-bearing tool step against the supplied tool map.
///
/// Split out of [`gate_check`] so each function keeps a single branch shape;
/// `gate_check`'s loop plus this four-way lookup in one body is the specific
/// PMAT cognitive-complexity risk in this module.
fn gate_check_step(
    step: &str,
    tool: &str,
    tools: &HashMap<String, Option<ToolAnnotations>>,
) -> Option<ProjectionWarning> {
    match tools.get(tool) {
        // Annotated, and annotated side-effecting: the SC-6 finding.
        Some(Some(annotations)) if is_annotated_side_effecting(annotations) => {
            Some(ProjectionWarning::new(
                ProjectionWarningKind::GuidanceOnSideEffectingStep,
                Some(step.to_string()),
                Some(tool.to_string()),
                format!(
                    "step `{step}` carries guidance, but its tool `{tool}` is annotated \
                     side-effecting; server-side workflow execution runs this step \
                     regardless of the guidance, so the guidance is a post-hoc judgment \
                     the executing surface will ignore"
                ),
            ))
        },
        // Annotated, and not annotated side-effecting: nothing to report.
        Some(Some(_)) => None,
        // Present but unannotated, or absent entirely — the same class: the
        // check could not be performed (D-08).
        Some(None) | None => Some(ProjectionWarning::new(
            ProjectionWarningKind::GateCheckUnverifiable,
            Some(step.to_string()),
            Some(tool.to_string()),
            format!(
                "step `{step}` carries guidance, but the side-effect check could not be \
                 performed: tool `{tool}` carries no annotations in the supplied tool map, \
                 so whether server-side execution would run this step regardless of the \
                 guidance is unknown"
            ),
        )),
    }
}

/// The SC-6 gate check: guidance attached to an annotated side-effecting step.
///
/// # Why this is a warning at all
///
/// Server-side workflow execution runs every deterministic step regardless of
/// its guidance prose, so guidance attached to a step that will run anyway is a
/// post-hoc judgment the executing surface ignores. That is the workflow
/// surface's blind spot, and the projection is where it becomes visible.
///
/// # The trigger is structural, never textual (D-09)
///
/// There is no phrase list and no analysis of the guidance prose. The claim the
/// warning makes is unconditionally true without reading the text — guidance IS
/// prose the executing surface ignores — so a heuristic over the wording could
/// only be paraphrased around, where this cannot.
///
/// # `tools == None` means the check does not run (D-07)
///
/// A `None` map is "no tool map was supplied", which is what a bare
/// [`SequentialWorkflow::as_skill`](crate::server::workflow::SequentialWorkflow::as_skill)
/// has; it returns no warnings at all. An EMPTY map is "a map was supplied and
/// this tool is not in it", which is unverifiable, not silent. The two states
/// are deliberately not collapsed.
///
/// Steps whose `tool()` is `None` are excluded entirely: they execute no tool
/// and can carry no annotations.
fn gate_check(
    wf: &SequentialWorkflow,
    tools: Option<&HashMap<String, Option<ToolAnnotations>>>,
) -> Vec<ProjectionWarning> {
    let Some(tools) = tools else {
        return Vec::new();
    };
    wf.steps()
        .iter()
        .filter(|step| step.guidance().is_some())
        .filter_map(|step| {
            step.tool()
                .and_then(|handle| gate_check_step(step.name().as_str(), handle.name(), tools))
        })
        .collect()
}

/// What [`SkillProjection::build`] returns on success: the projected skill plus
/// every warning the projection could observe.
///
/// # Why a named struct rather than a tuple or a bare `Skill`
///
/// D-02's literal wording says `build() -> Result<Skill>`, but D-10 requires
/// `build()` to return structured warnings, and a `Skill` cannot carry a
/// warning vector — the two locked decisions are incompatible as written. This
/// struct resolves them: it is named and self-documenting where a tuple is
/// positional, and being a struct it stays ADDITIVE, so a later phase can add a
/// field (a manifest, a strict verdict) without a breaking signature change.
/// `#[non_exhaustive]` makes that additivity real for downstream crates, which
/// read these fields but never construct the type.
#[derive(Debug)]
#[non_exhaustive]
pub struct ProjectionOutput {
    /// The projected skill. Its body is byte-identical to
    /// [`SequentialWorkflow::as_skill`]'s for the same workflow — both entry
    /// points share one renderer (D-05).
    ///
    /// [`SequentialWorkflow::as_skill`]: crate::server::workflow::SequentialWorkflow::as_skill
    pub skill: Skill,
    /// Every warning the projection could observe, in step order. Empty when no
    /// tool map was supplied (D-07).
    pub warnings: Vec<ProjectionWarning>,
}

impl ProjectionOutput {
    /// Consume this output and return both halves.
    ///
    /// This is the only way a DOWNSTREAM crate can move both fields out at
    /// once: `#[non_exhaustive]` forbids struct-pattern destructuring outside
    /// the defining crate, so `let ProjectionOutput { skill, warnings } = out;`
    /// does not compile there. Reading the fields individually still works.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "skills")] {
    /// use pmcp::server::skills::SkillProjection;
    /// use pmcp::server::workflow::SequentialWorkflow;
    ///
    /// let workflow = SequentialWorkflow::new("refund_flow", "Process a refund");
    /// let (skill, warnings) = SkillProjection::new(&workflow)
    ///     .build()
    ///     .expect("a legal name and a non-empty description")
    ///     .into_parts();
    ///
    /// assert_eq!(skill.name(), "refund-flow");
    /// // No tool map was supplied, so the SC-6 gate check did not run (D-07).
    /// assert!(warnings.is_empty());
    /// # }
    /// ```
    #[must_use]
    pub fn into_parts(self) -> (Skill, Vec<ProjectionWarning>) {
        (self.skill, self.warnings)
    }
}

/// The fallible, checking counterpart to
/// [`SequentialWorkflow::as_skill`](crate::server::workflow::SequentialWorkflow::as_skill).
///
/// `as_skill()` is infallible and resolves problems on the author's behalf.
/// This builder rejects them instead, and additionally performs the SC-6 gate
/// check when a tool map is supplied.
///
/// # A bare `as_skill()` cannot warn, and that is by construction
///
/// The SC-6 gate check needs [`ToolAnnotations`], which live only on
/// [`crate::types::ToolInfo::annotations`]. No workflow type carries them —
/// [`WorkflowStep::tool`] returns a [`ToolHandle`](crate::server::workflow::ToolHandle),
/// which is a name. A projection that receives only a `&SequentialWorkflow`
/// therefore cannot COMPUTE whether any step's tool is destructive; it is not
/// that it declines to report it. Supply a tool map through
/// [`Self::with_tools`] to get the check.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "skills")] {
/// use pmcp::server::skills::SkillProjection;
/// use pmcp::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};
///
/// let workflow = SequentialWorkflow::new("refund_flow", "Process a refund")
///     .step(WorkflowStep::new("fetch_order", ToolHandle::new("orders_get")));
///
/// let output = SkillProjection::new(&workflow)
///     .build()
///     .expect("a legal name and a non-empty description");
///
/// assert_eq!(output.skill.name(), "refund-flow");
/// assert_eq!(output.skill.body(), workflow.as_skill().body());
/// # }
/// ```
#[derive(Debug)]
pub struct SkillProjection<'w> {
    workflow: &'w SequentialWorkflow,
    /// `None` means no tool map was supplied and the gate check does not run at
    /// all (D-07). An empty map means one WAS supplied and every guidance-
    /// bearing step is unverifiable (D-08). Collapsing the two states would
    /// erase that distinction.
    tools: Option<HashMap<String, Option<ToolAnnotations>>>,
}

impl<'w> SkillProjection<'w> {
    /// Start a projection of `workflow`.
    ///
    /// No tool map is attached, so [`Self::build`] performs no SC-6 gate check
    /// and returns no gate warnings. **SC-6 gate warnings are delivered only
    /// through `build()`'s structured return, and only when
    /// [`Self::with_tools`] supplied annotations** — a bare
    /// [`SequentialWorkflow::as_skill`](crate::server::workflow::SequentialWorkflow::as_skill)
    /// holds no tool map and therefore cannot warn about a destructive step at
    /// all. A caller who expected a warning and got silence wants
    /// [`Self::with_tools`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "skills")] {
    /// use pmcp::server::skills::SkillProjection;
    /// use pmcp::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};
    ///
    /// let workflow = SequentialWorkflow::new("refund_flow", "Process a refund")
    ///     .step(WorkflowStep::new("fetch_order", ToolHandle::new("orders_get")));
    ///
    /// let output = SkillProjection::new(&workflow).build().expect("legal workflow");
    ///
    /// assert_eq!(output.skill.name(), "refund-flow");
    /// // No tool map, so no gate check ran (D-07).
    /// assert!(output.warnings.is_empty());
    /// # }
    /// ```
    #[must_use]
    pub fn new(workflow: &'w SequentialWorkflow) -> Self {
        Self {
            workflow,
            tools: None,
        }
    }

    /// Supply the tool annotations the SC-6 gate check needs.
    ///
    /// Takes [`crate::types::ToolInfo`] — the ONLY `ToolInfo` in this crate
    /// that carries `annotations`. `crate::server::workflow::ToolInfo` has
    /// three fields and structurally cannot, which is exactly why the check is
    /// a builder-path capability rather than something the workflow surface
    /// could do for itself.
    ///
    /// The `IntoIterator` form accepts a `Vec<ToolInfo>` straight off a
    /// `tools/list` result, a `ToolsCapabilities` listing, or a hand-written
    /// fixture; a `&HashMap` parameter would force callers to build a map they
    /// do not otherwise need. Later entries win on a duplicate name.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "skills")] {
    /// use pmcp::server::skills::SkillProjection;
    /// use pmcp::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};
    /// use pmcp::types::{ToolAnnotations, ToolInfo};
    /// use serde_json::json;
    ///
    /// let workflow = SequentialWorkflow::new("refund_flow", "Process a refund")
    ///     .step(WorkflowStep::new("fetch_order", ToolHandle::new("orders_get")));
    ///
    /// let tools = vec![ToolInfo::with_annotations(
    ///     "orders_get",
    ///     None,
    ///     json!({ "type": "object" }),
    ///     ToolAnnotations::new().with_read_only(true),
    /// )];
    ///
    /// let output = SkillProjection::new(&workflow)
    ///     .with_tools(tools)
    ///     .build()
    ///     .expect("legal workflow");
    ///
    /// // A read-only tool trips nothing, and supplying a tool map never moves
    /// // a rendered byte.
    /// assert!(output.warnings.is_empty());
    /// assert_eq!(output.skill.body(), workflow.as_skill().body());
    /// # }
    /// ```
    ///
    /// With a step that carries guidance and a tool annotated destructive, the
    /// SC-6 gate fires:
    ///
    /// ```rust
    /// # #[cfg(feature = "skills")] {
    /// use pmcp::server::skills::{ProjectionWarningKind, SkillProjection};
    /// use pmcp::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};
    /// use pmcp::types::{ToolAnnotations, ToolInfo};
    /// use serde_json::json;
    ///
    /// let workflow = SequentialWorkflow::new("refund_flow", "Process a refund").step(
    ///     WorkflowStep::new("issue_refund", ToolHandle::new("payments_refund"))
    ///         .with_guidance("Confirm with the customer before issuing the refund."),
    /// );
    ///
    /// let output = SkillProjection::new(&workflow)
    ///     .with_tools(vec![ToolInfo::with_annotations(
    ///         "payments_refund",
    ///         None,
    ///         json!({ "type": "object" }),
    ///         ToolAnnotations::new().with_destructive(true),
    ///     )])
    ///     .build()
    ///     .expect("legal workflow");
    ///
    /// assert_eq!(output.warnings.len(), 1);
    /// assert_eq!(
    ///     output.warnings[0].kind(),
    ///     ProjectionWarningKind::GuidanceOnSideEffectingStep
    /// );
    /// assert_eq!(output.warnings[0].step(), Some("issue_refund"));
    /// # }
    /// ```
    #[must_use]
    pub fn with_tools(mut self, tools: impl IntoIterator<Item = crate::types::ToolInfo>) -> Self {
        self.tools = Some(
            tools
                .into_iter()
                .map(|tool| (tool.name, tool.annotations))
                .collect(),
        );
        self
    }

    /// Render the projection, rejecting what
    /// [`SequentialWorkflow::as_skill`](crate::server::workflow::SequentialWorkflow::as_skill)
    /// would silently resolve.
    ///
    /// Both entry points call the SAME renderer, so the returned skill's body
    /// is byte-identical to `as_skill()`'s for any workflow that builds at all
    /// (D-05). The difference is disposition, never bytes.
    ///
    /// # Return shape — a recorded deviation from D-02's literal wording
    ///
    /// D-02 says `build() -> Result<Skill>` while D-10 says `build()` returns
    /// structured warnings; a `Skill` cannot carry a warning vector, so the two
    /// cannot both hold. This ships [`ProjectionOutput`] — additive under a
    /// future field, and named rather than positional. See that type's rustdoc.
    ///
    /// # Errors
    ///
    /// Returns `Err(pmcp::Error::Validation)` in exactly two cases:
    ///
    /// 1. **The workflow name normalizes to no agentskills-legal skill name**
    ///    (D-15) — every character maps out of `[a-z0-9-]`, or truncation
    ///    leaves nothing. `as_skill()` substitutes a deterministic
    ///    `workflow-{8 hex}` slug for the same input; this pushes a strict
    ///    caller to rename the workflow instead.
    /// 2. **The workflow description is empty** after trimming.
    ///    `SequentialWorkflow::new` validates nothing, so an empty description
    ///    is legal input, but `description:` with nothing after it renders a
    ///    YAML null and violates agentskills' non-empty rule. `as_skill()`
    ///    substitutes a deterministic legal string for the same input.
    ///
    /// No description LENGTH bound is checked. SEP-2640 states the digest
    /// format, the per-skill file and byte limits and the name-identity rule,
    /// but states NO description length limit, so inventing one here would be a
    /// guess; the registry's existing over-limit warning already covers a
    /// pathologically large body.
    ///
    /// This method never panics — it returns `Err` rather than panicking inside
    /// a `Result`-returning function (D-15).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "skills")] {
    /// use pmcp::server::skills::SkillProjection;
    /// use pmcp::server::workflow::SequentialWorkflow;
    ///
    /// let good = SequentialWorkflow::new("refund_flow", "Process a refund");
    /// assert!(SkillProjection::new(&good).build().is_ok());
    ///
    /// // Nothing in this name survives normalization.
    /// let nameless = SequentialWorkflow::new("!!!", "Process a refund");
    /// assert!(SkillProjection::new(&nameless).build().is_err());
    ///
    /// // The infallible path takes the same input and substitutes instead.
    /// assert!(nameless.as_skill().name().starts_with("workflow-"));
    ///
    /// // So does an empty description.
    /// let undescribed = SequentialWorkflow::new("refund_flow", "");
    /// assert!(SkillProjection::new(&undescribed).build().is_err());
    /// assert_eq!(
    ///     undescribed.as_skill().resolved_description(),
    ///     "Projected from the refund-flow workflow."
    /// );
    /// # }
    /// ```
    pub fn build(self) -> Result<ProjectionOutput> {
        let workflow = self.workflow;
        let name = workflow.name();

        if slugify(name).is_none() {
            return Err(Error::validation(format!(
                "workflow {name:?} normalizes to no agentskills-legal skill name; \
                 rename it to something containing at least one `[a-z0-9]` character, \
                 or use `SequentialWorkflow::as_skill()`, which substitutes a \
                 deterministic `workflow-{{8 hex}}` slug instead"
            )));
        }

        if description_is_absent(workflow) {
            return Err(Error::validation(format!(
                "workflow {name:?} has an empty description, which renders an \
                 agentskills-illegal frontmatter value; write a description, or use \
                 `SequentialWorkflow::as_skill()`, which substitutes a deterministic \
                 legal one instead"
            )));
        }

        // ONE renderer, not two: this wraps the same crate-private seam
        // `as_skill()` wraps. Both strict conditions were just rejected, so the
        // seam has nothing left to substitute and returns no notices.
        let (skill, notices) = project_with_notices(workflow);
        debug_assert!(
            notices.is_empty(),
            "build() rejects both substitution conditions before rendering, so the \
             shared seam cannot have substituted anything here"
        );

        let warnings = gate_check(workflow, self.tools.as_ref());

        // D-10, as narrowed: the BUILDER path delivers both channels — the
        // structured return above AND a log record on the module's existing
        // `mcp.skills` target. It is the only path that can, because it is the
        // only path holding a tool map. `as_skill()` logs the two conditions it
        // CAN observe (the slug fallback and the empty-description
        // substitution) and never reaches this check at all.
        //
        // Every author-supplied string is neutralized first (T-126-01):
        // workflow, step and tool names are author text that reaches a log
        // sink, and the message embeds the step and tool names. The STRUCTURED
        // return keeps the raw text; only the log record is neutralized.
        for warning in &warnings {
            tracing::warn!(
                target: "mcp.skills",
                workflow = %sanitize_for_log(name),
                step = %sanitize_for_log(warning.step().unwrap_or_default()),
                tool = %sanitize_for_log(warning.tool().unwrap_or_default()),
                "{}",
                sanitize_for_log(warning.message())
            );
        }

        Ok(ProjectionOutput { skill, warnings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Only the test below reads the predicate directly: production code in this
    // module reaches it through `sanitize_for_log`, and the encoder's escape
    // table spells the two characters out by codepoint.
    use crate::shared::log_sanitize::is_unicode_line_separator;

    use crate::server::workflow::{
        DataSource, InternalPromptMessage, PromptContent, ToolHandle, WorkflowStep,
    };
    use crate::types::{PromptArgumentType, Role};
    use proptest::prelude::*;
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::sync::OnceLock;

    /// The characters `yaml_double_quoted` treats specially, plus the blank
    /// whose interaction with them is the corruption half of CR-01.
    ///
    /// `' '` earns its place here rather than being left to chance: a raw
    /// `U+2028` only loses data when a blank is ADJACENT to it, so a generator
    /// that samples the separator but never a neighbouring space cannot
    /// observe failure mode 2.
    const ENCODER_SIGNIFICANT_CHARS: &[char] = &[
        '\\', '"', '\n', '\r', '\t', '\u{0}', '\u{7f}', '\u{85}', '\u{2028}', '\u{2029}', ' ', '-',
        '.', ':', '#',
    ];

    /// Text that can actually REACH the escape classes `yaml_double_quoted`
    /// exists for (WR-02).
    ///
    /// The three properties below previously drew from `".*"`. `.` in
    /// `regex-syntax` — which proptest compiles with default flags, so no
    /// `dot_matches_new_line` — is `[^\n]`, so that generator can NEVER produce
    /// `\n`, the single character the newline-injection defence rests on. It
    /// reaches `U+2028` only with probability ~1e-6 per character, i.e. never
    /// within a default 256-case run. That blindness is why CR-01 survived
    /// every guard the phase built.
    ///
    /// This strategy interleaves uniform draws from
    /// [`ENCODER_SIGNIFICANT_CHARS`] with `proptest`'s arbitrary `char`, so
    /// each escape class is sampled a constant fraction of the time and
    /// adjacent pairs (`<LS>` next to a blank; `<LS>` before `---`) occur
    /// within a normal run. The arbitrary half is kept so the property still
    /// covers the pass-through path and arbitrary non-ASCII.
    fn encoder_stressing_text() -> impl Strategy<Value = String> {
        proptest::collection::vec(
            prop_oneof![
                3 => proptest::sample::select(ENCODER_SIGNIFICANT_CHARS),
                2 => proptest::char::any(),
            ],
            0..24usize,
        )
        .prop_map(|chars| chars.into_iter().collect())
    }

    /// A `crate::types::ToolInfo` carrying annotations.
    ///
    /// `ToolAnnotations` and `ToolInfo` are both `#[non_exhaustive]`, so they
    /// are built through their constructors here and everywhere else — never
    /// with struct-literal syntax.
    fn annotated_tool(name: &str, annotations: ToolAnnotations) -> crate::types::ToolInfo {
        crate::types::ToolInfo::with_annotations(
            name,
            None,
            json!({ "type": "object" }),
            annotations,
        )
    }

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

    /// WR-06: the two non-`Cc` line separators are forged-record vectors too.
    #[test]
    fn sanitize_for_log_replaces_the_unicode_line_separators() {
        assert_eq!(sanitize_for_log("a\u{2028}b"), "a\u{fffd}b");
        assert_eq!(sanitize_for_log("a\u{2029}b"), "a\u{fffd}b");
        // Neither is Unicode `Cc`, which is why `is_control()` alone missed
        // them — assert the premise so this test cannot pass for a stale
        // reason if `char::is_control` ever widens.
        assert!(!'\u{2028}'.is_control());
        assert!(!'\u{2029}'.is_control());
    }

    /// CR-01/WR-06: the encoder and the log sanitizer classify the same set.
    ///
    /// They substitute DIFFERENT things — an escape versus `U+FFFD` — but a
    /// character either can terminate a line for a downstream consumer or it
    /// cannot, and that judgement must be made in exactly one place.
    #[test]
    fn the_encoder_and_the_log_sanitizer_agree_on_the_line_separators() {
        for c in ['\u{2028}', '\u{2029}'] {
            assert!(
                is_unicode_line_separator(c),
                "{c:?} must be classified as a line separator"
            );
            assert_eq!(
                sanitize_for_log(&c.to_string()),
                "\u{fffd}",
                "the log sanitizer must replace {c:?}"
            );
            assert!(
                !yaml_double_quoted(&c.to_string()).contains(c),
                "the encoder must not emit {c:?} verbatim"
            );
        }
        // Anti-vacuity: an ordinary character is classified by NEITHER, so the
        // three assertions above are not trivially true of every input.
        assert!(!is_unicode_line_separator('a'));
        assert_eq!(sanitize_for_log("a"), "a");
        assert!(yaml_double_quoted("a").contains('a'));
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
    fn frontmatter_survives_a_line_separator_before_a_document_indicator() {
        // CR-01, failure mode 1: raw, libyaml scans the LS as a line break and
        // then reads `--- ` at column 1 of the next line as a document
        // indicator, failing the parse. `FrontmatterParse::Invalid` is
        // DOWNGRADED to a diagnostic, so the skill vanishes from `skills/list`
        // with a successful `into_handler()`.
        assert_description_roundtrips("a\u{2028}--- b");
        assert_description_roundtrips("a\u{2029}--- b");
        // `...` is the other document indicator libyaml recognises.
        assert_description_roundtrips("a\u{2028}... b");
        assert_description_roundtrips("a\u{2029}... b");
    }

    #[test]
    fn frontmatter_survives_a_line_separator_adjacent_to_blanks() {
        // CR-01, failure mode 2: raw, the blanks on either side of the break
        // fold away and the published `frontmatter.description` silently
        // diverges from `Skill::resolved_description()`.
        assert_description_roundtrips("a\u{2028}   b");
        assert_description_roundtrips("a   \u{2028}b");
        assert_description_roundtrips("a\u{2029}   b");
        assert_description_roundtrips("a   \u{2029}b");
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
        // CR-01: the two YAML 1.1 line breaks that are not Unicode `Cc`. The
        // spelling is `\uNNNN`, never `\xNN` — libyaml's `\x` takes exactly two
        // hex digits, so `\x2028` would decode as a space plus the text `28`.
        assert_eq!(yaml_double_quoted("a\u{2028}b"), "\"a\\u2028b\"");
        assert_eq!(yaml_double_quoted("a\u{2029}b"), "\"a\\u2029b\"");
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

    // ── SC-2: determinism ─────────────────────────────────────────────

    /// Build a workflow whose single step's template bindings are inserted in
    /// the given order. Everything else about the two workflows is identical.
    fn workflow_with_binding_order(order: [&str; 3]) -> SequentialWorkflow {
        let mut step = WorkflowStep::new("issue_refund", ToolHandle::new("payments_refund"))
            .arg("amount", DataSource::from_step_field("order", "total"))
            .bind("refund");
        for key in order {
            step = step.with_template_binding(key, DataSource::prompt_arg(key));
        }
        SequentialWorkflow::new("refund_flow", "Process a customer refund")
            .argument("order_id", "The order to refund", true)
            .step(step)
    }

    /// The PRIMARY SC-2 proof (REVIEWS codex).
    ///
    /// Two workflows differing ONLY in the insertion order of their template
    /// bindings must render byte-equal bodies. Deterministic, single-run,
    /// cannot flake, and fails 100% of the time against an unsorted `HashMap`
    /// iteration — which is what a repeated-reconstruction loop cannot promise,
    /// since it only fails when the seeds happen to disagree.
    ///
    /// Three keys, not two: a two-element `HashMap` can iterate in the "right"
    /// order by accident often enough to make the test a coin flip.
    #[test]
    fn sc2_binding_insertion_order_does_not_change_bytes() {
        let forward = workflow_with_binding_order(["zeta", "middle", "alpha"])
            .as_skill()
            .body()
            .to_string();
        let reversed = workflow_with_binding_order(["alpha", "middle", "zeta"])
            .as_skill()
            .body()
            .to_string();

        // Anti-vacuity: two empty bodies are also byte-equal.
        assert!(
            forward.contains("- Template variable `alpha`:")
                && forward.contains("- Template variable `middle`:")
                && forward.contains("- Template variable `zeta`:"),
            "SC-2 anti-vacuity: all three template bindings must actually render; \
             body was:\n{forward}"
        );

        assert_eq!(
            forward, reversed,
            "SC-2: template-binding INSERTION order must not reach the rendered \
             bytes — `template_bindings()` returns a `&HashMap` and is the one \
             nondeterministic accessor in the input surface, so `render_step` \
             collects it into a `BTreeMap` first"
        );
    }

    /// The body of the FIRST kitchen-sink render in this process.
    ///
    /// `prop_sc2_rerender_is_byte_equal` compares every later render against
    /// this one, which is what "byte-equal across re-derivations" means.
    fn first_kitchen_sink_body() -> &'static str {
        static FIRST: OnceLock<String> = OnceLock::new();
        FIRST.get_or_init(|| kitchen_sink_workflow().as_skill().body().to_string())
    }

    proptest! {
        /// SUPPLEMENTAL SC-2 proof.
        ///
        /// CONSTRUCTING A FRESH workflow inside the loop body is load-bearing:
        /// Rust's `HashMap` randomizes per `RandomState`, which is per map
        /// INSTANCE, so re-rendering one instance often hash-iterates
        /// identically within a process and then differs across processes or
        /// after a rehash. Rendering one instance twice is the shape of a
        /// determinism test that cannot fail.
        ///
        /// `kitchen_sink_workflow()` is used because it carries template
        /// bindings — the only nondeterministic input.
        #[test]
        fn prop_sc2_rerender_is_byte_equal(_n in 0..100u32) {
            let fresh = kitchen_sink_workflow().as_skill().body().to_string();
            prop_assert!(fresh.len() > 400, "anti-vacuity: body was {} bytes", fresh.len());
            prop_assert_eq!(fresh.as_str(), first_kitchen_sink_body());
        }
    }

    // ── SC-1: slug legality as a property over arbitrary names ────────

    proptest! {
        /// SC-1 (property half): whatever `as_skill()` produces for an
        /// arbitrary workflow name, the result is an agentskills-legal skill
        /// name — 1..=64 characters drawn from `[a-z0-9-]`, with no leading or
        /// trailing hyphen and no `--`.
        ///
        /// Drawn from [`encoder_stressing_text`], not `".*"`: the latter
        /// cannot produce `\n` at all and reaches `U+2028` only by accident
        /// (WR-02).
        #[test]
        fn prop_sc1_slug_is_agentskills_legal(name in encoder_stressing_text()) {
            let skill = SequentialWorkflow::new(name.as_str(), "Process a refund").as_skill();
            let slug = skill.name();
            prop_assert!(!slug.is_empty(), "slug was empty for name {:?}", name);
            prop_assert!(
                slug.len() <= MAX_SLUG_LEN,
                "slug {:?} is {} chars, over the {} bound",
                slug,
                slug.len(),
                MAX_SLUG_LEN
            );
            prop_assert!(
                slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "slug {:?} left the [a-z0-9-] alphabet",
                slug
            );
            prop_assert!(!slug.starts_with('-'), "slug {:?} has a leading hyphen", slug);
            prop_assert!(!slug.ends_with('-'), "slug {:?} has a trailing hyphen", slug);
            prop_assert!(!slug.contains("--"), "slug {:?} has a doubled hyphen", slug);
        }

        /// SC-1: the same name always projects to the same slug, across two
        /// freshly constructed workflows.
        #[test]
        fn prop_sc1_slug_is_deterministic(name in ".*") {
            let first = SequentialWorkflow::new(name.as_str(), "Process a refund")
                .as_skill()
                .name()
                .to_string();
            let second = SequentialWorkflow::new(name.as_str(), "Process a refund")
                .as_skill()
                .name()
                .to_string();
            prop_assert_eq!(first, second);
        }

        /// D-15: nothing panics. Arbitrary text reaches the name, the
        /// description, an argument description, an instruction and a step's
        /// guidance — every author-supplied string the renderer touches.
        ///
        /// "Arbitrary" is honoured by [`encoder_stressing_text`], which mixes
        /// `proptest::char::any()` with deliberate draws from the escape
        /// classes. The previous `".*"` generator made the word false: it can
        /// never emit `\n` (WR-02).
        #[test]
        fn prop_no_panic_on_arbitrary_text(
            name in encoder_stressing_text(),
            description in encoder_stressing_text(),
            guidance in encoder_stressing_text(),
        ) {
            let wf = SequentialWorkflow::new(name.as_str(), description.as_str())
                .argument("arg", description.as_str(), true)
                .instruction(InternalPromptMessage::new(
                    Role::User,
                    PromptContent::Text(description.clone()),
                ))
                .step(
                    WorkflowStep::new("step", ToolHandle::new("tool"))
                        .with_guidance(guidance.as_str())
                        .bind("out"),
                );
            let skill = wf.as_skill();
            prop_assert!(!skill.body().is_empty());
            prop_assert!(skill.body().ends_with('\n'));
        }
    }

    // ── SC-5: the dual surface ────────────────────────────────────────

    /// SC-5: the SEP-2640 skill text and the prompt text are ONE content.
    ///
    /// The guards run BEFORE the equality on purpose. `as_prompt_text()`
    /// returns `body` unchanged only when the body already ends in exactly one
    /// newline AND the skill carries zero references; both are renderer
    /// guarantees, not free properties, and the equality passes vacuously on an
    /// empty body.
    #[test]
    fn sc5_prompt_text_equals_body() {
        let skill = kitchen_sink_workflow().as_skill();
        assert_eq!(
            skill.references().count(),
            0,
            "SC-5 holds only for a reference-free skill; `as_prompt_text()` \
             appends every reference body"
        );
        assert!(
            skill.body().ends_with('\n'),
            "SC-5 needs the body to end in a newline; `as_prompt_text()` appends one"
        );
        assert!(
            !skill.body().ends_with("\n\n"),
            "SC-5 needs EXACTLY one trailing newline"
        );
        assert!(
            skill.body().len() > 200,
            "SC-5 anti-vacuity: body was only {} bytes",
            skill.body().len()
        );
        assert_eq!(skill.as_prompt_text(), skill.body());
    }

    /// Slice `body` to its `## Procedure` section, exclusive of the next
    /// `## ` heading.
    ///
    /// Scoping the scan is required, not cosmetic (REVIEWS fable): a
    /// `PromptContent::ToolHandle` instruction renders a backticked tool NAME
    /// into `## Context`, so an unscoped scan over the whole body could pick up
    /// a name the Procedure never called and fail the set equality for a reason
    /// that is not a defect.
    fn procedure_section(body: &str) -> &str {
        const HEADING: &str = "## Procedure";
        let start = body
            .find(HEADING)
            .expect("the rendered body must carry a Procedure section");
        let rest = &body[start + HEADING.len()..];
        rest.find("\n## ").map_or(rest, |end| &rest[..end])
    }

    /// SC-5 (surface equivalence): the SET of tool names in the rendered
    /// Procedure EQUALS the set the workflow declares.
    ///
    /// Set equality in BOTH directions. One-way containment would pass a
    /// renderer that emitted a tool name the workflow never declared, or that
    /// silently dropped one.
    ///
    /// The first set is derived by parsing the RENDERED text, not by re-walking
    /// the workflow — parsing the workflow twice would prove nothing about the
    /// render.
    #[test]
    fn sc5_surface_equivalence_is_set_equality() {
        let wf = kitchen_sink_workflow();
        let skill = wf.as_skill();
        let procedure = procedure_section(skill.body());
        assert!(
            procedure.contains("### Step 1:"),
            "anti-vacuity: the Procedure slice must carry steps; slice was:\n{procedure}"
        );

        let rendered: BTreeSet<&str> = procedure
            .lines()
            .filter_map(|line| line.strip_prefix("Call tool `"))
            .filter_map(|rest| rest.split_once('`').map(|(name, _)| name))
            .collect();
        let declared: BTreeSet<&str> = wf
            .steps()
            .iter()
            .filter_map(WorkflowStep::tool)
            .map(ToolHandle::name)
            .collect();

        assert!(
            !declared.is_empty(),
            "anti-vacuity: the fixture must declare at least one tool"
        );
        assert!(
            rendered.is_subset(&declared),
            "the render named a tool the workflow never declared: rendered={rendered:?}, \
             declared={declared:?}"
        );
        assert!(
            declared.is_subset(&rendered),
            "the render dropped a declared tool: rendered={rendered:?}, \
             declared={declared:?}"
        );
        assert_eq!(rendered, declared);
    }

    /// WR-02, made checkable: the generator REACHES the characters it claims.
    ///
    /// A widened generator that quietly narrows again would leave the three
    /// properties above passing while measuring nothing — the failure shape
    /// this repository has recorded before. Drawing from the strategy directly
    /// and asserting the escape classes appear turns "can sample" from a
    /// comment into a test. `\n` is the character the old `".*"` could never
    /// produce; `U+2028`/`U+2029` are the ones it reached at ~1e-6.
    #[test]
    fn the_stressing_generator_reaches_every_escape_class() {
        use proptest::strategy::ValueTree;
        use proptest::test_runner::{Config, TestRunner};

        let strategy = encoder_stressing_text();
        let mut runner = TestRunner::new(Config::default());
        let mut corpus = String::new();
        for _ in 0..512 {
            corpus.push_str(
                &strategy
                    .new_tree(&mut runner)
                    .expect("the strategy must produce a value")
                    .current(),
            );
        }

        for (label, needle) in [
            ("a newline", '\n'),
            ("LINE SEPARATOR", '\u{2028}'),
            ("PARAGRAPH SEPARATOR", '\u{2029}'),
            ("a backslash", '\\'),
            ("a double quote", '"'),
            ("a blank", ' '),
        ] {
            assert!(
                corpus.contains(needle),
                "the generator never produced {label} ({needle:?}) in 512 draws — \
                 the properties drawing from it are measuring nothing"
            );
        }

        // Anti-vacuity for the corpus itself: a strategy that returned one
        // giant fixed string would satisfy every `contains` above.
        assert!(
            corpus.chars().filter(|c| !c.is_ascii()).count() > 0,
            "the arbitrary half of the strategy produced no non-ASCII at all"
        );
    }

    // ── REVIEWS finding 1: the round-trip property ────────────────────

    proptest! {
        /// The encoder is verified by the decoder the registry actually uses.
        ///
        /// Drawn from [`encoder_stressing_text`]. Under the previous `".*"`
        /// generator this property was structurally incapable of finding
        /// CR-01, and equally incapable of exercising the `'\n'` arm it was
        /// written to protect (WR-02); `prop_frontmatter_roundtrips` was cited
        /// in the module header as "a real check rather than a mirror of the
        /// encoder's own assumptions", which it was not.
        #[test]
        fn prop_frontmatter_roundtrips(description in encoder_stressing_text()) {
            let wf = tracer_workflow("refund_flow", &description);
            let skill = wf.as_skill();
            let value = parsed_frontmatter(skill.body());
            let obj = value.as_object().expect("frontmatter must parse to a mapping");
            prop_assert_eq!(obj.len(), 2);
            prop_assert_eq!(obj.get("name").and_then(|v| v.as_str()), Some("refund-flow"));
            let parsed_description = obj.get("description").and_then(|v| v.as_str());
            // `trim()`, not `is_empty()`: a description of `"\n"` or `"   "` is
            // BLANK, and the substitution branch is what the infallible path
            // takes for it — `description_is_absent` is the one predicate both
            // entry points share. Testing `is_empty()` here asserted a verbatim
            // round-trip of a blank scalar, i.e. exactly the agentskills-illegal
            // frontmatter value the substitution exists to prevent.
            if description.trim().is_empty() {
                prop_assert_eq!(
                    parsed_description,
                    Some("Projected from the refund-flow workflow.")
                );
            } else {
                prop_assert_eq!(parsed_description, Some(description.as_str()));
            }
        }
    }

    // ── Plan 126-04 Task 1: the fallible builder path (D-01 / D-02) ───

    #[test]
    fn build_and_as_skill_share_one_renderer() {
        let wf = kitchen_sink_workflow();
        let output = SkillProjection::new(&wf)
            .build()
            .expect("a legal name and a non-empty description must build");
        assert_eq!(
            output.skill.body(),
            wf.as_skill().body(),
            "D-05: the fallible and infallible entry points must share ONE renderer"
        );
        assert_eq!(output.skill.name(), wf.as_skill().name());
        assert_eq!(
            output.skill.resolved_description(),
            wf.as_skill().resolved_description()
        );
    }

    #[test]
    fn build_without_a_tool_map_emits_no_warnings() {
        // The kitchen-sink fixture carries a guidance-bearing step whose tool
        // WOULD trip the SC-6 gate if a tool map were supplied.
        let wf = kitchen_sink_workflow();
        let output = SkillProjection::new(&wf).build().expect("legal workflow");
        assert!(
            output.warnings.is_empty(),
            "D-07: with no tool map there is nothing to check, so nothing to warn about; got {:?}",
            output.warnings
        );
    }

    #[test]
    fn build_rejects_a_name_that_normalizes_to_nothing() {
        let wf = tracer_workflow("!!!", "Process a refund");
        let err = SkillProjection::new(&wf)
            .build()
            .expect_err("D-15: the strict path rejects a name with nothing legal in it");
        let message = err.to_string();
        assert!(
            message.contains("!!!"),
            "the error must name the offending workflow; got {message:?}"
        );
    }

    #[test]
    fn as_skill_falls_back_where_build_rejects_the_name() {
        // Same input, two dispositions (D-15): `build()` errors, `as_skill()`
        // substitutes and keeps going.
        let wf = tracer_workflow("!!!", "Process a refund");
        let skill = wf.as_skill();
        let hex = skill
            .name()
            .strip_prefix("workflow-")
            .expect("the infallible path substitutes a `workflow-{8 hex}` slug");
        assert_eq!(hex.len(), 8, "slug was {:?}", skill.name());
        assert!(
            hex.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
            "slug was {:?}",
            skill.name()
        );
    }

    #[test]
    fn build_rejects_an_empty_description() {
        let wf = tracer_workflow("refund_flow", "");
        let err = SkillProjection::new(&wf)
            .build()
            .expect_err("REVIEWS finding 6: an empty description is rejected by the strict path");
        let message = err.to_string();
        assert!(
            message.contains("description"),
            "the error must name the empty description; got {message:?}"
        );
        assert!(
            message.contains("refund_flow"),
            "the error must name the offending workflow; got {message:?}"
        );
    }

    #[test]
    fn build_rejects_a_whitespace_only_description() {
        let wf = tracer_workflow("refund_flow", "   \t  ");
        let err = SkillProjection::new(&wf)
            .build()
            .expect_err("an all-whitespace description is empty after trimming");
        assert!(err.to_string().contains("description"));
    }

    #[test]
    fn as_skill_substitutes_where_build_rejects_the_description() {
        let wf = tracer_workflow("refund_flow", "");
        let skill = wf.as_skill();
        assert_eq!(
            skill.resolved_description(),
            "Projected from the refund-flow workflow.",
            "the pinned substitution from plan 126-01 Task 4"
        );
        let value = parsed_frontmatter(skill.body());
        assert_eq!(
            value.get("description").and_then(|v| v.as_str()),
            Some("Projected from the refund-flow workflow.")
        );
    }

    /// The whitespace-only case of the property above, as its own regression.
    ///
    /// `build()` has always rejected `"   \t  "` (see
    /// `build_rejects_a_whitespace_only_description` two tests up), while
    /// `resolve_description` substituted only on the UNTRIMMED `is_empty()` — so
    /// the infallible path rendered `description: "   \t  "` into a digested
    /// frontmatter block and emitted NO notice. The test named
    /// `as_skill_substitutes_where_build_rejects_the_description` covered only
    /// `""`, so it was true of its own name for one of the two rejected shapes.
    #[test]
    fn as_skill_substitutes_for_a_whitespace_only_description_too() {
        for blank in ["   \t  ", "\n", " "] {
            let wf = tracer_workflow("refund_flow", blank);
            let skill = wf.as_skill();
            assert_eq!(
                skill.resolved_description(),
                "Projected from the refund-flow workflow.",
                "a blank description must take the same substitution `build()` rejects it for; \
                 input was {blank:?}"
            );
            let value = parsed_frontmatter(skill.body());
            assert_eq!(
                value.get("description").and_then(|v| v.as_str()),
                Some("Projected from the refund-flow workflow."),
                "input was {blank:?}"
            );
            assert!(
                SkillProjection::new(&wf).build().is_err(),
                "the anti-vacuity half: `build()` must still REJECT {blank:?}, so the two entry \
                 points differ in disposition over one shared condition rather than in the \
                 condition itself"
            );
        }

        // A description that merely CONTAINS whitespace is untouched.
        let kept = tracer_workflow("refund_flow", "  Process a refund  ");
        assert_eq!(
            kept.as_skill().resolved_description(),
            "  Process a refund  "
        );
    }

    /// `U+FFFE` and `U+FFFF` are escaped, so the rendered block still parses.
    ///
    /// Neither is Unicode `Cc` and neither is a line separator, so both slipped
    /// past every arm of the encoder and reached the frontmatter raw — where
    /// YAML 1.1's printable set (`U+E000..=U+FFFD` in the BMP) rejects them, the
    /// block fails to parse, and the registry silently drops the skill from
    /// `skills/list` while `into_handler()` still returns `Ok`. That is the
    /// CR-01 class the encoder's own comment block claims to have removed.
    #[test]
    fn the_bmp_noncharacters_survive_the_frontmatter_round_trip() {
        for description in ["Refund \u{FFFF} orders", "Refund \u{FFFE} orders"] {
            let wf = tracer_workflow("refund_flow", description);
            let body = wf.as_skill().body().to_string();
            // Only the FRONTMATTER block is asserted: it is the half a YAML
            // reader sees. The `# ` heading below the closing `---` is markdown
            // and carries the author's character verbatim, which is correct —
            // `parse_frontmatter_value` never hands those bytes to libyaml.
            let frontmatter_block = body
                .split("\n---\n")
                .next()
                .expect("split always yields a first element");
            assert!(
                !frontmatter_block.contains('\u{FFFE}') && !frontmatter_block.contains('\u{FFFF}'),
                "the noncharacter must be ESCAPED in the frontmatter, not emitted raw: \
                 {frontmatter_block:?}"
            );
            let value = parsed_frontmatter(&body);
            assert_eq!(
                value.get("description").and_then(|v| v.as_str()),
                Some(description),
                "the escaped form must decode back to the author's exact text"
            );
        }
    }

    #[test]
    fn with_tools_records_a_map_and_still_builds_the_same_bytes() {
        let wf = kitchen_sink_workflow();
        let tools = vec![annotated_tool(
            "orders_get",
            ToolAnnotations::new().with_read_only(true),
        )];
        let output = SkillProjection::new(&wf)
            .with_tools(tools)
            .build()
            .expect("legal workflow");
        assert_eq!(
            output.skill.body(),
            wf.as_skill().body(),
            "supplying a tool map must not move a single rendered byte"
        );
    }

    #[test]
    fn projection_output_into_parts_returns_both_halves() {
        let wf = kitchen_sink_workflow();
        let (skill, warnings) = SkillProjection::new(&wf)
            .build()
            .expect("legal workflow")
            .into_parts();
        assert_eq!(skill.name(), "refund-flow");
        assert!(warnings.is_empty());
    }

    // ── Plan 126-04 Task 2: the SC-6 gate warning (D-08 / D-09) ───────

    /// A `crate::types::ToolInfo` with `annotations == None`.
    fn unannotated_tool(name: &str) -> crate::types::ToolInfo {
        crate::types::ToolInfo::new(name, None, json!({ "type": "object" }))
    }

    /// One tool step named `issue_refund` calling `payments_refund`, with or
    /// without guidance.
    fn one_step_workflow(guidance: Option<&str>) -> SequentialWorkflow {
        let mut step = WorkflowStep::new("issue_refund", ToolHandle::new("payments_refund"));
        if let Some(text) = guidance {
            step = step.with_guidance(text);
        }
        SequentialWorkflow::new("refund_flow", "Process a refund").step(step)
    }

    fn warnings_for(
        wf: &SequentialWorkflow,
        tools: Vec<crate::types::ToolInfo>,
    ) -> Vec<ProjectionWarning> {
        SkillProjection::new(wf)
            .with_tools(tools)
            .build()
            .expect("legal workflow")
            .warnings
    }

    #[test]
    fn gate_fires_for_guidance_on_a_destructive_tool() {
        let wf = one_step_workflow(Some("Confirm with the customer first."));
        let warnings = warnings_for(
            &wf,
            vec![annotated_tool(
                "payments_refund",
                ToolAnnotations::new().with_destructive(true),
            )],
        );
        assert_eq!(warnings.len(), 1, "got {warnings:?}");
        assert_eq!(
            warnings[0].kind(),
            ProjectionWarningKind::GuidanceOnSideEffectingStep
        );
        assert_eq!(warnings[0].step(), Some("issue_refund"));
        assert_eq!(warnings[0].tool(), Some("payments_refund"));
        assert!(
            warnings[0].message().contains("issue_refund"),
            "the message must name the step: {:?}",
            warnings[0].message()
        );
        assert!(
            warnings[0].message().contains("regardless"),
            "the message must say the step runs regardless of the guidance: {:?}",
            warnings[0].message()
        );
    }

    #[test]
    fn gate_fires_for_guidance_on_an_explicitly_not_read_only_tool() {
        let wf = one_step_workflow(Some("Confirm with the customer first."));
        let warnings = warnings_for(
            &wf,
            vec![annotated_tool(
                "payments_refund",
                ToolAnnotations::new().with_read_only(false),
            )],
        );
        assert_eq!(warnings.len(), 1, "got {warnings:?}");
        assert_eq!(
            warnings[0].kind(),
            ProjectionWarningKind::GuidanceOnSideEffectingStep
        );
    }

    #[test]
    fn gate_stays_silent_for_guidance_on_a_read_only_tool() {
        let wf = one_step_workflow(Some("Confirm with the customer first."));
        let warnings = warnings_for(
            &wf,
            vec![annotated_tool(
                "payments_refund",
                ToolAnnotations::new().with_read_only(true),
            )],
        );
        assert_eq!(warnings.len(), 0, "got {warnings:?}");
    }

    #[test]
    fn an_unannotated_tool_is_unverifiable_not_a_gate_finding() {
        let wf = one_step_workflow(Some("Confirm with the customer first."));
        let warnings = warnings_for(&wf, vec![unannotated_tool("payments_refund")]);
        assert_eq!(warnings.len(), 1, "got {warnings:?}");
        assert_eq!(
            warnings[0].kind(),
            ProjectionWarningKind::GateCheckUnverifiable,
            "D-08: MCP's literal annotation defaults are NOT followed"
        );
        assert_eq!(
            warnings
                .iter()
                .filter(|w| w.kind() == ProjectionWarningKind::GuidanceOnSideEffectingStep)
                .count(),
            0
        );
    }

    #[test]
    fn a_tool_absent_from_the_map_is_unverifiable() {
        let wf = one_step_workflow(Some("Confirm with the customer first."));
        let warnings = warnings_for(
            &wf,
            vec![annotated_tool("orders_get", ToolAnnotations::new())],
        );
        assert_eq!(warnings.len(), 1, "got {warnings:?}");
        assert_eq!(
            warnings[0].kind(),
            ProjectionWarningKind::GateCheckUnverifiable
        );
        assert_eq!(warnings[0].tool(), Some("payments_refund"));
    }

    #[test]
    fn a_destructive_tool_without_guidance_produces_nothing() {
        let wf = one_step_workflow(None);
        let warnings = warnings_for(
            &wf,
            vec![annotated_tool(
                "payments_refund",
                ToolAnnotations::new().with_destructive(true),
            )],
        );
        assert_eq!(
            warnings.len(),
            0,
            "the trigger is guidance AND a side effect, never a side effect alone; got {warnings:?}"
        );
    }

    #[test]
    fn a_resource_only_step_with_guidance_produces_nothing() {
        let wf = SequentialWorkflow::new("refund_flow", "Process a refund").step(
            WorkflowStep::fetch_resources("read_policy")
                .with_resource("docs://refund-policy")
                .expect("valid resource URI")
                .with_guidance("Read the policy before deciding."),
        );
        let warnings = warnings_for(
            &wf,
            vec![annotated_tool(
                "payments_refund",
                ToolAnnotations::new().with_destructive(true),
            )],
        );
        assert_eq!(
            warnings.len(),
            0,
            "a step that calls no tool can carry no annotations; got {warnings:?}"
        );
    }

    #[test]
    fn without_a_tool_map_nothing_warns_even_for_a_tripping_workflow() {
        // The SAME workflow that produces a gate warning WITH a map.
        let wf = one_step_workflow(Some("Confirm with the customer first."));
        let with_map = warnings_for(
            &wf,
            vec![annotated_tool(
                "payments_refund",
                ToolAnnotations::new().with_destructive(true),
            )],
        );
        assert_eq!(with_map.len(), 1, "anti-vacuity: the fixture must trip");

        let without_map = SkillProjection::new(&wf)
            .build()
            .expect("legal workflow")
            .warnings;
        assert_eq!(
            without_map.len(),
            0,
            "D-07: no tool map means the check cannot run at all; got {without_map:?}"
        );
    }

    #[test]
    fn two_side_effecting_guidance_bearing_steps_produce_exactly_two_warnings() {
        let wf = SequentialWorkflow::new("refund_flow", "Process a refund")
            .step(
                WorkflowStep::new("issue_refund", ToolHandle::new("payments_refund"))
                    .with_guidance("Confirm with the customer first."),
            )
            .step(
                WorkflowStep::new("close_order", ToolHandle::new("orders_close"))
                    .with_guidance("Only close once the refund settled."),
            )
            .step(WorkflowStep::new(
                "fetch_order",
                ToolHandle::new("orders_get"),
            ));
        let warnings = warnings_for(
            &wf,
            vec![
                annotated_tool(
                    "payments_refund",
                    ToolAnnotations::new().with_destructive(true),
                ),
                annotated_tool("orders_close", ToolAnnotations::new().with_read_only(false)),
                annotated_tool("orders_get", ToolAnnotations::new().with_read_only(true)),
            ],
        );
        assert_eq!(warnings.len(), 2, "got {warnings:?}");
        assert!(warnings
            .iter()
            .all(|w| w.kind() == ProjectionWarningKind::GuidanceOnSideEffectingStep));
        let steps: Vec<_> = warnings
            .iter()
            .filter_map(ProjectionWarning::step)
            .collect();
        assert_eq!(steps, vec!["issue_refund", "close_order"]);
    }

    // ── Plan 126-04 Task 3: D-10's narrowed delivery contract ─────────

    #[test]
    fn an_author_supplied_name_is_neutralized_before_it_reaches_a_log_field() {
        // T-126-01: a workflow name carrying a newline and an ESC could forge a
        // second log record or move a terminal cursor.
        let hostile = "refund\u{1b}[2Kflow\ninjected: line";
        let sanitized = sanitize_for_log(hostile);
        assert!(!sanitized.contains('\n'), "newline survived: {sanitized:?}");
        assert!(!sanitized.contains('\u{1b}'), "ESC survived: {sanitized:?}");
        assert!(sanitized.contains("refund"), "content was destroyed");
        assert_eq!(
            sanitized.matches('\u{fffd}').count(),
            2,
            "each control character maps to exactly one U+FFFD: {sanitized:?}"
        );
    }

    #[test]
    fn build_returns_warning_kinds_and_counts_as_data() {
        // D-10 exists so tests assert on DATA — no `tracing` subscriber is
        // installed anywhere in this suite.
        let wf = SequentialWorkflow::new("refund_flow", "Process a refund")
            .step(
                WorkflowStep::new("issue_refund", ToolHandle::new("payments_refund"))
                    .with_guidance("Confirm with the customer first."),
            )
            .step(
                WorkflowStep::new("archive_order", ToolHandle::new("orders_archive"))
                    .with_guidance("Only archive settled orders."),
            );
        let warnings = warnings_for(
            &wf,
            vec![annotated_tool(
                "payments_refund",
                ToolAnnotations::new().with_destructive(true),
            )],
        );
        assert_eq!(warnings.len(), 2, "got {warnings:?}");
        let kinds: Vec<_> = warnings.iter().map(ProjectionWarning::kind).collect();
        assert_eq!(
            kinds,
            vec![
                ProjectionWarningKind::GuidanceOnSideEffectingStep,
                ProjectionWarningKind::GateCheckUnverifiable,
            ]
        );
    }

    #[test]
    fn the_warning_is_a_builder_capability_and_the_bytes_are_identical_either_way() {
        // The observable form of the D-10 narrowing: for the SAME workflow the
        // builder reports a gate finding and `as_skill()` reports nothing, yet
        // neither entry point moves a single rendered byte.
        let wf = one_step_workflow(Some("Confirm with the customer first."));
        let output = SkillProjection::new(&wf)
            .with_tools(vec![annotated_tool(
                "payments_refund",
                ToolAnnotations::new().with_destructive(true),
            )])
            .build()
            .expect("legal workflow");

        assert!(
            output
                .warnings
                .iter()
                .any(|w| w.kind() == ProjectionWarningKind::GuidanceOnSideEffectingStep),
            "anti-vacuity: the fixture must trip the gate"
        );
        assert_eq!(
            output.skill.body(),
            wf.as_skill().body(),
            "the warning is a builder-path capability; the bytes are identical either way"
        );
    }
}
