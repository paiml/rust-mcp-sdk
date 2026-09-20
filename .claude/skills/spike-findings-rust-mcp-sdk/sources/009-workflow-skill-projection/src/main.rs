//! Spike 009: workflow → skill projection.
//!
//! Positioning thesis under test: a `SequentialWorkflow` and a SEP-2640
//! Skill are two surfaces of ONE artifact. The workflow surface executes a
//! deterministic prefix server-side (`prompts/get`); the skill surface
//! teaches a client LLM to run the same procedure itself with the server's
//! tools (`skills/*` + `resources/read`).
//!
//! Kill-risk: a mechanically derived SKILL.md could be lossy garbage. The
//! spike builds the projection, then measures (a) coverage — everything the
//! workflow knows lands in the text, (b) determinism, (c) discoverability
//! through the shipped `Skills` registry, and (d) surface equivalence — the
//! tools the workflow prompt actually executes are exactly the tools the
//! skill names.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::skills::{Skill, Skills};
use pmcp::server::workflow::dsl::{field, from_step, prompt_arg};
use pmcp::server::workflow::{
    DataSource, InternalPromptMessage, SequentialWorkflow, ToolHandle, WorkflowPromptHandler,
    WorkflowStep,
};
use pmcp::server::{PromptHandler, ResourceHandler, ToolHandler};
use pmcp::types::content::Role;
use pmcp::RequestHandlerExtra;
use serde_json::{json, Value};

const RULE: &str = "──────────────────────────────────────────────────────────";

// ── Demo tools (in-process, deterministic) ────────────────────────────

struct GetOrder;
#[async_trait]
impl ToolHandler for GetOrder {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let id = args.get("id").and_then(Value::as_str).unwrap_or("?");
        Ok(json!({ "id": id, "total": 42.50, "status": "delivered" }))
    }
}

struct CheckEligibility;
#[async_trait]
impl ToolHandler for CheckEligibility {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let status = args
            .pointer("/order/status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        Ok(json!({ "eligible": status == "delivered", "reason": "within 30-day window" }))
    }
}

struct IssueRefund;
#[async_trait]
impl ToolHandler for IssueRefund {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({ "refund_id": "R-1001", "refunded": args.get("amount").cloned().unwrap_or(Value::Null) }))
    }
}

// ── The projection under test ─────────────────────────────────────────

/// Slugify a workflow name into an agentskills-legal skill name
/// (lowercase, alnum + hyphens; the name becomes the URI's final segment).
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn render_data_source(ds: &DataSource) -> String {
    match ds {
        DataSource::PromptArg(a) => format!("use the `{}` argument you were given", a.as_str()),
        DataSource::StepOutput { step, field: None } => {
            format!("use the entire saved `{}` result", step.as_str())
        },
        DataSource::StepOutput {
            step,
            field: Some(f),
        } => format!("use field `{f}` of the saved `{}` result", step.as_str()),
        DataSource::Constant(v) => format!("use the constant value `{v}`"),
        _ => "(unrenderable data source)".to_string(),
    }
}

/// Project a `SequentialWorkflow` into a SEP-2640 `Skill`.
///
/// Everything in the body is derived from workflow introspection — steps,
/// tools, argument data-flow, bindings, guidance, workflow-level
/// instructions. The final section cross-references the server-accelerated
/// prompt path WITHOUT redirecting (the manual procedure is complete on its
/// own — the dual-surface rule's spirit, extended to a third surface).
fn project_skill(wf: &SequentialWorkflow) -> Skill {
    let name = slugify(wf.name());
    let mut b = String::new();
    b.push_str(&format!(
        "---\nname: {name}\ndescription: {}\n---\n# {}\n\n{}\n",
        wf.description(),
        wf.name(),
        wf.description()
    ));

    if !wf.instructions().is_empty() {
        b.push_str("\n## Context\n\n");
        for m in wf.instructions() {
            match &m.content {
                pmcp::server::workflow::PromptContent::Text(t) => {
                    b.push_str(t);
                    b.push_str("\n\n");
                },
                other => b.push_str(&format!("{other:?}\n\n")),
            }
        }
    }

    b.push_str("\n## Inputs\n\n");
    for (arg_name, spec) in wf.arguments() {
        b.push_str(&format!(
            "- `{}`{}: {}\n",
            arg_name.as_str(),
            if spec.required { " (required)" } else { "" },
            spec.description
        ));
    }

    b.push_str(
        "\n## Procedure\n\nRun these steps in order. Each names the MCP tool \
         to call ON THIS SERVER and how to build its arguments.\n",
    );
    for (i, step) in wf.steps().iter().enumerate() {
        b.push_str(&format!("\n### Step {}: {}\n", i + 1, step.name().as_str()));
        if let Some(tool) = step.tool() {
            b.push_str(&format!("Call tool `{}`.\n", tool.name()));
        }
        if !step.arguments().is_empty() {
            b.push_str("Arguments:\n");
            for (arg, ds) in step.arguments() {
                b.push_str(&format!("- `{}`: {}\n", arg.as_str(), render_data_source(ds)));
            }
        }
        if let Some(binding) = step.binding() {
            b.push_str(&format!("Save the result as `{}`.\n", binding.as_str()));
        }
        if let Some(g) = step.guidance() {
            b.push_str(&format!("Judgment: {g}\n"));
        }
    }

    b.push_str(&format!(
        "\n## Server-accelerated alternative\n\nThis server also exposes the MCP \
         prompt `{}`. Fetching that prompt runs the deterministic steps above \
         server-side and returns their results inline, leaving only the judgment \
         calls to you. Prefer it when your host supports MCP prompts; the manual \
         procedure above is complete on its own when it does not.\n",
        wf.name()
    ));

    Skill::new(name, b)
}

// ── Demo workflow ─────────────────────────────────────────────────────

fn build_workflow(with_guidance: bool) -> SequentialWorkflow {
    let issue = WorkflowStep::new("issue_refund", ToolHandle::new("issue_refund"))
        .arg("id", field("order", "id"))
        .arg("amount", field("order", "total"))
        .bind("refund");
    let issue = if with_guidance {
        issue.with_guidance(
            "Only issue the refund if `eligibility.eligible` is true. If not, do NOT \
             call the tool — draft a polite decline note citing `eligibility.reason`.",
        )
    } else {
        issue
    };
    SequentialWorkflow::new(
        "refund_flow",
        "Process a customer refund request per company policy",
    )
    .argument("order_id", "The order to refund", true)
    .instruction(InternalPromptMessage::new(
        Role::System,
        "You are a support agent handling refunds. Be precise about amounts.",
    ))
    .step(
        WorkflowStep::new("fetch_order", ToolHandle::new("get_order"))
            .arg("id", prompt_arg("order_id"))
            .bind("order"),
    )
    .step(
        WorkflowStep::new("check_eligibility", ToolHandle::new("check_eligibility"))
            .arg("order", from_step("order"))
            .bind("eligibility"),
    )
    .step(issue)
}

fn tool_registry() -> (
    HashMap<Arc<str>, pmcp::server::workflow::ToolInfo>,
    HashMap<Arc<str>, Arc<dyn ToolHandler>>,
) {
    let mut infos = HashMap::new();
    let mut handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();
    for (name, desc, handler) in [
        ("get_order", "Fetch an order by id", Arc::new(GetOrder) as Arc<dyn ToolHandler>),
        (
            "check_eligibility",
            "Check refund eligibility for an order",
            Arc::new(CheckEligibility),
        ),
        ("issue_refund", "Issue a refund for an order", Arc::new(IssueRefund)),
    ] {
        infos.insert(
            Arc::from(name),
            pmcp::server::workflow::ToolInfo {
                name: name.to_string(),
                description: desc.to_string(),
                input_schema: json!({ "type": "object" }),
            },
        );
        handlers.insert(Arc::from(name), handler);
    }
    (infos, handlers)
}

// ── Steps ─────────────────────────────────────────────────────────────

fn step_1_project_and_show() -> Skill {
    println!("{RULE}");
    println!("Step 1 — the projection: derived SKILL.md");
    println!("{RULE}");
    let wf = build_workflow(true);
    let skill = project_skill(&wf);
    println!("{}", skill.body());
    skill
}

fn step_2_coverage_and_determinism(skill: &Skill) {
    println!("{RULE}");
    println!("Step 2 — coverage + determinism assertions");
    println!("{RULE}");
    let wf = build_workflow(true);
    let body = skill.body();

    for step in wf.steps() {
        assert!(body.contains(step.name().as_str()), "step name in body");
        if let Some(t) = step.tool() {
            assert!(body.contains(&format!("`{}`", t.name())), "tool name in body");
        }
        if let Some(bind) = step.binding() {
            assert!(body.contains(bind.as_str()), "binding in body");
        }
        if let Some(g) = step.guidance() {
            assert!(body.contains(g), "guidance verbatim in body");
        }
        for (arg, _) in step.arguments() {
            assert!(body.contains(arg.as_str()), "step arg in body");
        }
    }
    for (arg, spec) in wf.arguments() {
        assert!(body.contains(arg.as_str()) && body.contains(&spec.description));
    }
    println!("  ✓ every step, tool, binding, argument, and guidance string lands in the text");

    let again = project_skill(&build_workflow(true));
    assert_eq!(skill.body(), again.body());
    println!("  ✓ projection is deterministic (byte-equal on re-derivation)");

    assert_eq!(skill.name(), "refund-flow");
    println!("  ✓ workflow name `refund_flow` slugified to skill name `refund-flow`");
    println!("    (finding: workflow names are NOT automatically agentskills-legal —");
    println!("     underscores must map to hyphens; projection owns that rule)");

    let bare = project_skill(&build_workflow(false));
    let with_g = skill.body().len();
    println!(
        "  ◆ guidance is the quality dial: {} bytes with guidance vs {} without —",
        with_g,
        bare.body().len()
    );
    println!("    the mechanical skeleton is valid but thin; `with_guidance` text is");
    println!("    where judgment (the LLM's half) actually comes from.");
    assert!(!bare.body().contains("Judgment:"));
}

async fn step_3_discoverability(skill: Skill) -> anyhow::Result<()> {
    println!("\n{RULE}");
    println!("Step 3 — derived skill flows through the shipped Skills registry");
    println!("{RULE}");
    let handler = Skills::new().add(skill.clone()).into_handler()?;
    let listed = handler.list(None, RequestHandlerExtra::default()).await?;
    let uris: Vec<&str> = listed.resources.iter().map(|r| r.uri.as_str()).collect();
    assert!(uris.contains(&"skill://refund-flow/SKILL.md"));
    println!("  ✓ discoverable at skill://refund-flow/SKILL.md (URI final segment == name)");

    let read = handler
        .read("skill://refund-flow/SKILL.md", RequestHandlerExtra::default())
        .await?;
    let wire = serde_json::to_value(&read)?;
    assert_eq!(
        wire.pointer("/contents/0/text").and_then(Value::as_str),
        Some(skill.body())
    );
    println!("  ✓ resources/read returns the projected body byte-identical");

    assert_eq!(skill.as_prompt_text(), skill.body());
    println!("  ✓ tri-surface fallback: as_prompt_text() == body (no references), so");
    println!("    bootstrap_skill_and_prompt would inline the identical text — the");
    println!("    dual-surface invariant holds for projected skills for free");
    Ok(())
}

async fn step_4_surface_equivalence(skill: &Skill) -> anyhow::Result<()> {
    println!("\n{RULE}");
    println!("Step 4 — workflow prompt still executes; surfaces name the same tools");
    println!("{RULE}");
    let (infos, handlers) = tool_registry();
    let wf = build_workflow(true);
    wf.validate().map_err(|e| anyhow::anyhow!("validate: {e}"))?;
    let ph = WorkflowPromptHandler::new(wf, infos, handlers, None::<Arc<dyn ResourceHandler>>);

    let mut args = HashMap::new();
    args.insert("order_id".to_string(), "ORD-77".to_string());
    let result = ph.handle(args, RequestHandlerExtra::default()).await?;

    println!("  prompts/get returned {} messages:", result.messages.len());
    let wire = serde_json::to_value(&result)?;
    let all_text = wire.to_string();
    for (i, m) in wire.pointer("/messages").and_then(Value::as_array).unwrap().iter().enumerate() {
        let role = m.pointer("/role").and_then(Value::as_str).unwrap_or("?");
        let text = m
            .pointer("/content/text")
            .and_then(Value::as_str)
            .unwrap_or("(non-text)");
        let preview: String = text.chars().take(110).collect();
        println!("    [{i}] {role}: {}{}", preview.replace('\n', " "), if text.len() > 110 { "…" } else { "" });
    }

    // Partial tool results really executed server-side:
    assert!(all_text.contains("ORD-77"), "get_order executed with the prompt arg");
    assert!(all_text.contains("42.5"), "order total flowed through data-flow bindings");
    println!("  ✓ deterministic steps executed server-side; partial results are inline");

    // Surface equivalence: every tool the prompt surface names, the skill names.
    for tool in ["get_order", "check_eligibility", "issue_refund"] {
        assert!(all_text.contains(tool), "prompt surface references {tool}");
        assert!(skill.body().contains(tool), "skill surface references {tool}");
    }
    println!("  ✓ surface equivalence: both surfaces name exactly the same three tools");

    // Observe: did the server execute issue_refund despite the guidance?
    if all_text.contains("R-1001") {
        println!("  ◆ OBSERVED: the server executed `issue_refund` server-side even though");
        println!("    the guidance says the LLM should decide. Guidance text is invisible");
        println!("    to server-side execution — it rides along as prose. A workflow whose");
        println!("    judgment must gate a side-effecting step should model the judgment");
        println!("    step EXPLICITLY (elicitation / no-tool step), or leave the step out");
        println!("    of the deterministic prefix. Positioning: this is precisely the line");
        println!("    between the workflow surface (server executes) and the skill surface");
        println!("    (LLM executes) — the projection makes the line legible.");
    } else {
        println!("  ◆ OBSERVED: `issue_refund` was NOT executed server-side — the handler");
        println!("    stopped ahead of it. Inspect messages above for how the handoff is");
        println!("    phrased; the skill surface must phrase the same handoff.");
    }
    Ok(())
}

fn print_verdict() {
    println!("\n{RULE}");
    println!("VERDICT");
    println!("{RULE}");
    println!(
        r#"
Projection is real, cheap, and faithful. ~90 lines derive a valid, discoverable
SKILL.md from a SequentialWorkflow with zero new SDK machinery: every fact the
workflow holds (steps, tools, data-flow, bindings, guidance, instructions)
lands in the text; re-derivation is byte-equal; the result flows through the
shipped Skills registry untouched; the fallback-prompt invariant holds free.

The thesis refinement the spike surfaced: the two surfaces are not equals with
different transports — they differ on WHERE JUDGMENT RUNS. Server-side
execution ignores guidance prose; the skill surface exists precisely to carry
it. So the projection's value is highest for workflows with meaningful
guidance, and the SDK API should say so:

  - `SequentialWorkflow::as_skill()` (or `Skills::from_workflow(&wf)`) — the
    ~90-line renderer, owned by the SDK so the surfaces cannot drift.
  - Slugification rule owned by the projection (workflow names are not
    agentskills-legal; `refund_flow` → `refund-flow`).
  - The generated "Server-accelerated alternative" section cross-references
    the prompt WITHOUT redirecting — manual procedure stays complete.
  - DX guardrail: a side-effecting step gated by guidance should trip a
    projection-time warning (the guidance can't gate server-side execution).
"#
    );
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("{RULE}");
    println!("Spike 009: workflow → skill projection");
    println!("{RULE}\n");
    let skill = step_1_project_and_show();
    step_2_coverage_and_determinism(&skill);
    step_3_discoverability(skill.clone()).await?;
    step_4_surface_equivalence(&skill).await?;
    print_verdict();
    Ok(())
}
