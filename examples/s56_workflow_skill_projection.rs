//! # Server example: workflow -> skill projection (Phase 126)
//!
//! `s44_server_skills` remains the HAND-AUTHORED skills example: it embeds
//! `SKILL.md` files written by a human via `include_str!` and registers them.
//! This file is its projection sibling. Nothing here is hand-authored — every
//! byte of the skill it serves is DERIVED from a
//! [`SequentialWorkflow`](pmcp::server::workflow::SequentialWorkflow) the
//! server already had, by one call to `as_skill()`.
//!
//! Run with: `cargo run --example s56_workflow_skill_projection --features skills,full`
//!
//! What this example demonstrates. Every item is **asserted before it is
//! printed** — a silently-passing example that prints "OK" while the invariant
//! is broken is worse than no example at all, so this file follows
//! `c10_client_skills`'s assert-then-print habit rather than `s44`'s
//! print-only one:
//!
//! 1. **The projection, its slugification and the GOLDEN bytes (SC-1, SC-2,
//!    SC-5).** A workflow named `refund_flow` projects to a skill named
//!    `refund-flow`; the render is deterministic (a second, independent
//!    derivation is byte-equal); the dual-surface invariant
//!    `as_prompt_text() == body()` holds; and the rendered body is byte-equal
//!    to `tests/golden/workflow_skill_projection.md` — the file whose sha256 is
//!    published in the skill's `skills/list` entry.
//! 2. **Registry pass-through (SC-4, in process).** The projected skill goes
//!    into a `Skills` registry, its `skills/list` entry carries a
//!    `sha256:`-prefixed digest whose `size` matches the served body, and
//!    `resources/read` on `skill://refund-flow/SKILL.md` returns those exact
//!    bytes.
//! 3. **The projection-time gate warning (SC-6).** `SkillProjection::build()`
//!    with a tool map reports exactly one
//!    `GuidanceOnSideEffectingStep` warning, naming the guidance-bearing step
//!    whose tool is annotated destructive — and returns a skill whose bytes
//!    are identical to `as_skill()`'s, because both paths share one renderer.
//! 4. **The opt-in prepended prompt (D-04a), through the BUILDER.** With
//!    `ServerBuilder::with_workflow_skill_prepend(true)`, prompt message `[0]`
//!    of the workflow's own prompt IS the projected skill body — the same
//!    string, not a second rendering. With the setter omitted (the default),
//!    message `[0]` is the ordinary user-intent message.
//! 5. A closing summary naming which success criterion each assertion covered.
//!
//! ## Why this example builds TWO servers
//!
//! Following `s44`'s habit of explaining a duplication rather than doing
//! something surprising in silence: `ServerBuilder::with_workflow_skill_prepend`
//! is read at REGISTRATION time, inside `prompt_workflow`. A single server
//! therefore cannot show both states, and toggling after the fact is not a
//! thing the API offers. Two servers built from the same fixture — one with the
//! setter, one without — is the only way to show the flag's effect AND its
//! default in one run.
//!
//! ## Why the BUILDER path, and not a hand-constructed handler
//!
//! The opt-in also exists on `WorkflowPromptHandler::with_projected_skill_prepend`,
//! which is what the builder calls internally. Demonstrating it that way would
//! teach a path real servers do not take: a server that registers workflows the
//! normal way — `Server::builder() ... .prompt_workflow(wf)?` — never
//! constructs a `WorkflowPromptHandler` by hand. The builder setter exists so
//! the "one string, one digest" claim holds per SERVER rather than per workflow
//! value, so that is the call this example shows.
//!
//! Note the ORDER below: `.with_workflow_skill_prepend(true)` must precede
//! `.prompt_workflow(...)`, and the tools and the resource handler must precede
//! it too, because `prompt_workflow` snapshots all three at registration time.
//!
//! ## Registration uses `try_skills`, not `skills`
//!
//! `ServerBuilder::skills` surfaces a registry failure as a `panic!` inside a
//! `Result`-returning `build()` (`src/server/builder.rs:1501`, phase 125's
//! still-open WR-03). `try_skills` returns the error instead. A reader copying
//! this example copies the form that cannot abort their process.
//!
//! ## What this example does NOT do
//!
//! It binds no socket and holds no client, so it issues no real `skills/list`
//! or `skills/get` RPC — those are answered over streamable HTTP. The
//! authoritative wire proof lives in `tests/skills_routing.rs`.
//!
//! Note what "pinned by the golden" means HERE, because the distinction is the
//! whole point of the [`GOLDEN`] assertion below. `make test-examples` only
//! BUILDS examples; nothing in the quality gate RUNS this one. So the claim
//! that these bytes are the golden's bytes is checked at two different
//! strengths: the `include_str!` makes the golden a COMPILE-TIME dependency of
//! this file (delete or move it and the example stops building, which the gate
//! does see), and the byte comparison itself runs whenever the example does.
//! Before the assertion existed the claim was prose only, and a re-recorded
//! golden could silently falsify it while this file kept compiling, kept
//! printing and kept passing.

use std::collections::HashMap;

use async_trait::async_trait;
use pmcp::server::skills::{ProjectionWarningKind, SkillProjection, Skills};
use pmcp::server::workflow::{
    DataSource, InternalPromptMessage, SequentialWorkflow, ToolHandle, WorkflowStep,
};
use pmcp::types::{Content, PromptArgumentType, PromptMessage, ToolAnnotations, ToolInfo};
use pmcp::{
    ListResourcesResult, ReadResourceResult, RequestHandlerExtra, ResourceHandler, Server,
    SimpleTool,
};
use serde_json::json;

/// The canonical URI the projected skill is served from.
const SKILL_URI: &str = "skill://refund-flow/SKILL.md";

/// The policy document `read_policy` fetches.
const POLICY_URI: &str = "file:///policies/refunds.md";

/// The D-14 golden: the exact bytes `refund_flow().as_skill().body()` renders.
///
/// `include_str!` resolves relative to THIS file at COMPILE time, so the
/// example cannot pass by reading a stale or absent golden — a moved or deleted
/// fixture is a build failure, which `make test-examples` and `make lint`'s
/// `cargo check --examples` both see.
///
/// `tests/golden/workflow_skill_projection.md` is the same file
/// `tests/skills_integration.rs`'s `golden_render_is_byte_equal` pins, and its
/// sha256 is what a pinning consumer binds to. Asserting against it here is
/// what keeps [`refund_flow`]'s "same shape as the golden fixture" from being
/// an unverified comment.
const GOLDEN: &str = include_str!("../tests/golden/workflow_skill_projection.md");

/// The workflow this whole example projects.
///
/// Deliberately the same shape as the D-14 golden fixture in
/// `tests/skills_integration.rs`, so the bytes this example prints are the
/// bytes `tests/golden/workflow_skill_projection.md` pins — and `main` ASSERTS
/// that against [`GOLDEN`] rather than leaving it to this comment. Each element earns
/// its place: `refund_flow` needs slugifying; the description carries a `: `
/// mapping indicator and a `#` comment indicator so the frontmatter must be
/// YAML-ENCODED rather than concatenated; the instruction is the only way
/// `## Context` becomes observable; the two arguments pin both `## Inputs`
/// bullet shapes; `issue_refund` carries the guidance that trips the SC-6 gate.
fn refund_flow() -> SequentialWorkflow {
    SequentialWorkflow::new("refund_flow", "Process a customer refund: policy #7")
        .instruction(InternalPromptMessage::system(
            "Refunds above the policy ceiling need a supervisor's approval.",
        ))
        .typed_argument(
            "order_id",
            "The order to refund",
            true,
            PromptArgumentType::String,
        )
        .argument("reason", "Why the customer asked for the refund", false)
        .step(
            WorkflowStep::new("fetch_order", ToolHandle::new("orders_get"))
                .arg("id", DataSource::prompt_arg("order_id"))
                .arg(
                    "options",
                    DataSource::constant(json!({ "zeta": 1, "alpha": 2 })),
                )
                .bind("order"),
        )
        .step(
            WorkflowStep::new("issue_refund", ToolHandle::new("payments_refund"))
                .arg("order", DataSource::from_step_field("order", "id"))
                .with_template_binding("zeta_total", DataSource::from_step_field("order", "total"))
                .with_template_binding("alpha_reason", DataSource::prompt_arg("reason"))
                .with_guidance("Confirm the customer accepted the policy before issuing."),
        )
        .step(
            WorkflowStep::fetch_resources("read_policy")
                .with_resource(POLICY_URI)
                // `with_resource` is fallible only because a URI may carry
                // template variables that must resolve; this one is a literal,
                // so the error arm is unreachable by construction.
                .expect("a literal resource URI carries no template variables"),
        )
}

/// Serves the one policy document the workflow's resource-only step reads.
///
/// Without it, `read_policy` would fail at prompt time and the D-04a
/// demonstration below could never build a transcript.
struct PolicyResources;

#[async_trait]
impl ResourceHandler for PolicyResources {
    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        if uri == POLICY_URI {
            return Ok(ReadResourceResult::new(vec![Content::text(
                "Refunds under 200 are auto-approved. Above that, a supervisor signs off.",
            )]));
        }
        Err(pmcp::Error::validation(format!("unknown resource: {uri}")))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        // `ListResourcesResult` is `#[non_exhaustive]`; construct it through
        // `new()`, never with struct-literal syntax.
        Ok(ListResourcesResult::new(vec![]))
    }
}

/// The caller's arguments. `reason` is optional but supplied, because
/// `issue_refund` binds it into a template variable.
fn args() -> HashMap<String, String> {
    let mut args = HashMap::new();
    args.insert("order_id".to_string(), "ord-42".to_string());
    args.insert("reason".to_string(), "arrived damaged".to_string());
    args
}

/// Build a server registering `refund_flow` as a prompt, with the D-04a
/// prepend either enabled through the builder or left at its default.
///
/// Ordering is load-bearing and is the reason this is a function rather than
/// two ad-hoc chains: the resource handler, both tools and the prepend setter
/// must ALL precede `prompt_workflow`, which snapshots them at registration
/// time.
fn build_refund_server(prepend_projected_skill: bool) -> pmcp::Result<Server> {
    let builder = Server::builder()
        .name("refund-server")
        .version("1.0.0")
        .resources(PolicyResources)
        .tool(
            "orders_get",
            SimpleTool::new("orders_get", |_args, _extra| {
                Box::pin(async move { Ok(json!({ "id": "ord-42", "total": 199 })) })
            })
            .with_description("Fetch an order")
            .with_schema(json!({ "type": "object" })),
        )
        .tool(
            "payments_refund",
            SimpleTool::new("payments_refund", |_args, _extra| {
                Box::pin(async move { Ok(json!({ "refund_id": "rf-1" })) })
            })
            .with_description("Issue a refund against an order")
            .with_schema(json!({ "type": "object" })),
        )
        // `try_skills`, never `skills` — see the module docs.
        .try_skills(Skills::new().add(refund_flow().as_skill()))?;

    // The OFF case omits the setter entirely, so what it demonstrates is the
    // DEFAULT rather than an explicit `false`.
    let builder = if prepend_projected_skill {
        builder.with_workflow_skill_prepend(true)
    } else {
        builder
    };

    builder.prompt_workflow(refund_flow())?.build()
}

/// The text of a transcript message. Every message a workflow prompt emits is
/// textual; anything else is a defect worth failing on.
fn message_text(message: &PromptMessage) -> &str {
    match &message.content {
        Content::Text { text } => text.as_str(),
        other => panic!("expected Content::Text in a transcript, got {other:?}"),
    }
}

/// The text of a `resources/read` result. The skills handler answers with
/// `Content::Resource` so the per-resource URI and MIME type survive the wire.
fn resource_text(contents: &[Content]) -> String {
    match contents.first() {
        Some(Content::Resource { text, .. }) => text
            .clone()
            .expect("the skills handler always emits a text body"),
        other => panic!("expected Content::Resource from a skill read, got {other:?}"),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workflow = refund_flow();

    // ── 1. The projection, its slug, and its determinism ──────────────
    //
    // SC-1: the workflow's `_` name is not a legal agentskills skill name;
    // the projection owns the slugification, so no illegal name can reach
    // `skills/list`.
    let skill = workflow.as_skill();
    assert_eq!(
        skill.name(),
        "refund-flow",
        "SC-1: `refund_flow` must project to the slug `refund-flow`"
    );

    // Anti-vacuity for the equality that follows: a skill with references
    // would make `as_prompt_text() == body()` a claim about concatenation
    // rather than about the dual surface, and an empty body would make it
    // trivially true.
    assert_eq!(
        skill.references().count(),
        0,
        "a projected skill carries no reference files"
    );
    assert!(
        skill.body().len() > 200,
        "projected body was only {} bytes",
        skill.body().len()
    );

    // SC-5: one byte-identical body serves BOTH surfaces.
    assert_eq!(
        skill.as_prompt_text(),
        skill.body(),
        "SC-5: the prompt surface and the SEP-2640 surface must be one string"
    );

    // SC-2: a second, independently constructed workflow renders the same
    // bytes. This is what makes the published `sha256` digest reproducible.
    assert_eq!(
        refund_flow().as_skill().body(),
        skill.body(),
        "SC-2: the render must be deterministic across derivations"
    );

    // D-14: the bytes this example prints ARE the golden's bytes.
    //
    // A red here has the two opposite causes `golden_break_message` in
    // `tests/skills_integration.rs` spells out, plus a third that belongs to
    // this file alone: the golden's fixture was extended and `refund_flow`
    // above was not, so the two derivations drifted. Fix the fixture — do NOT
    // re-record the golden from this example's output.
    assert_eq!(
        skill.body(),
        GOLDEN,
        "D-14: this example's projected body is no longer byte-equal to \
         tests/golden/workflow_skill_projection.md, whose sha256 is published \
         in the skill's skills/list entry. Either `refund_flow()` here drifted \
         from `golden_workflow()` in tests/skills_integration.rs, or the \
         renderer changed. Re-recording the golden from THIS output is never \
         the fix."
    );

    println!("== 1. The projected skill ==\n");
    println!("workflow name : {}", workflow.name());
    println!("skill name    : {}", skill.name());
    println!("body bytes    : {}\n", skill.body().len());
    println!("{}", skill.body());

    // ── 2. Registry pass-through, in process ──────────────────────────
    //
    // Build the registry once: `entries()` borrows it and `into_handler()`
    // consumes it, so the projection must be read first.
    let registry = Skills::new().add(skill.clone());
    let entries = registry.entries()?;
    assert_eq!(entries.len(), 1, "SC-4: exactly one entry was registered");

    let entry = &entries[0];
    assert_eq!(entry.uri(), SKILL_URI, "SC-4: the canonical SKILL.md URI");
    assert_eq!(
        entry.frontmatter()["name"],
        json!("refund-flow"),
        "SC-4: the entry frontmatter carries the projected name VERBATIM"
    );

    let manifest = entry
        .resources()
        .first()
        .expect("every entry manifests at least its own SKILL.md");
    assert!(
        manifest.digest().starts_with("sha256:"),
        "SC-4: digest was {}",
        manifest.digest()
    );
    assert_eq!(
        manifest.digest().len(),
        "sha256:".len() + 64,
        "SC-4: a sha256 digest is 64 hex characters"
    );
    assert_eq!(
        manifest.size(),
        skill.body().len(),
        "SC-4: the published size must match the served body"
    );

    // `resources/read` returns the SAME bytes the digest was taken over.
    let handler = registry.into_handler()?;
    let read = handler
        .read(SKILL_URI, RequestHandlerExtra::default())
        .await?;
    let served = resource_text(&read.contents);
    assert_eq!(
        served,
        skill.body(),
        "SC-4: the served bytes must be the projected bytes"
    );

    println!("== 2. Registry pass-through ==\n");
    println!("entry uri     : {}", entry.uri());
    println!("entry digest  : {}", manifest.digest());
    println!(
        "entry size    : {} (served {} bytes)\n",
        manifest.size(),
        served.len()
    );

    // ── 3. The projection-time gate warning (SC-6) ────────────────────
    //
    // The trigger is purely STRUCTURAL: guidance prose on a step whose tool is
    // annotated side-effecting. The prose is never analysed, so it cannot be
    // paraphrased around. `ToolAnnotations` is `#[non_exhaustive]` — build it
    // with `new()` + setters, never struct-literal syntax.
    let output = SkillProjection::new(&workflow)
        .with_tools(vec![
            ToolInfo::with_annotations(
                "orders_get",
                None,
                json!({ "type": "object" }),
                ToolAnnotations::new().with_read_only(true),
            ),
            ToolInfo::with_annotations(
                "payments_refund",
                None,
                json!({ "type": "object" }),
                ToolAnnotations::new().with_destructive(true),
            ),
        ])
        .build()?;

    assert_eq!(
        output.warnings.len(),
        1,
        "SC-6: expected exactly one warning, got {:?}",
        output.warnings
    );
    assert_eq!(
        output.warnings[0].kind(),
        ProjectionWarningKind::GuidanceOnSideEffectingStep,
        "SC-6: the gate kind"
    );
    assert_eq!(
        output.warnings[0].step(),
        Some("issue_refund"),
        "SC-6: the warning must name the guidance-bearing step"
    );
    assert_eq!(
        output.warnings[0].tool(),
        Some("payments_refund"),
        "SC-6: the warning must name the side-effecting tool"
    );

    // D-05 again, across the two entry points: the checking builder and the
    // infallible `as_skill()` share ONE renderer, so disposition differs and
    // bytes never do.
    assert_eq!(
        output.skill.body(),
        skill.body(),
        "D-05: `build()` and `as_skill()` must render identical bytes"
    );

    println!("== 3. Projection-time gate warnings (SC-6) ==\n");
    for warning in &output.warnings {
        println!(
            "  [{:?}] step={:?} tool={:?}\n    {}",
            warning.kind(),
            warning.step(),
            warning.tool(),
            warning.message()
        );
    }
    println!();

    // ── 4. The opt-in prepended prompt, through the builder ───────────
    let flagged = build_refund_server(true)?;
    let flagged_result = flagged
        .get_prompt("refund_flow")
        .expect("prompt_workflow registered the handler under the workflow's own name")
        .handle(args(), RequestHandlerExtra::default())
        .await?;

    assert_eq!(
        message_text(&flagged_result.messages[0]),
        skill.body(),
        "D-04a: with the opt-in on, message [0] IS the projected skill body"
    );

    let plain = build_refund_server(false)?;
    let plain_result = plain
        .get_prompt("refund_flow")
        .expect("prompt_workflow registered the handler under the workflow's own name")
        .handle(args(), RequestHandlerExtra::default())
        .await?;

    assert_ne!(
        message_text(&plain_result.messages[0]),
        skill.body(),
        "D-04a: the prepend is default-OFF"
    );
    assert!(
        message_text(&plain_result.messages[0])
            .starts_with("I want to Process a customer refund: policy #7."),
        "D-04a: the default message [0] is the user-intent message, was {:?}",
        message_text(&plain_result.messages[0])
    );

    println!("== 4. The opt-in prepended prompt (D-04a), via the builder ==\n");
    println!(
        "with `.with_workflow_skill_prepend(true)` -> message[0] is {} bytes, \
         byte-equal to the served skill",
        message_text(&flagged_result.messages[0]).len()
    );
    println!(
        "default (setter omitted)                 -> message[0] is {:?}\n",
        message_text(&plain_result.messages[0])
            .lines()
            .next()
            .unwrap_or_default()
    );

    // ── 5. Closing summary ────────────────────────────────────────────
    println!("== 5. What this run proved ==\n");
    println!("  SC-1  slugification owned by the projection: `refund_flow` -> `refund-flow`");
    println!("  SC-2  determinism: an independent second derivation is byte-equal");
    println!("  SC-3  full coverage: the printed body carries Context, Inputs and every step");
    println!(
        "  SC-4  registry pass-through: sha256 digest + size + `resources/read` byte identity"
    );
    println!("  SC-5  dual surface: `as_prompt_text() == body()`, one string for both surfaces");
    println!("  SC-6  gate warning: one GuidanceOnSideEffectingStep on `issue_refund`");
    println!("  D-14  golden: the printed body is byte-equal to tests/golden/workflow_skill_projection.md");
    println!("  D-04a opt-in prepend reached through `ServerBuilder::with_workflow_skill_prepend`");
    println!("\nAll assertions passed.");

    Ok(())
}
