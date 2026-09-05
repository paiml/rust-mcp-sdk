//! Spike 011: one task, three mechanisms — the measured decision matrix.
//!
//! The same refund task runs as:
//!   A. SKILL      — client LLM reads SKILL.md, executes the procedure itself
//!   B. WORKFLOW   — prompts/get pre-executes the steps server-side
//!   C. AGENT      — a delegated pmcp-agent loop runs it remotely
//!
//! Same domain tools everywhere. Surfaces A and C share the IDENTICAL
//! `decide()` policy function — hosted client-side in A and inside the
//! remote loop in C — so "where judgment runs" is literal shared code.
//!
//! Both an ELIGIBLE (ORD-77) and an INELIGIBLE (ORD-BAD, returned order)
//! scenario run, so spike 009's post-hoc-judgment observation becomes a
//! measured outcome: does a refund get issued when it should not?

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use pmcp::server::workflow::dsl::{field, from_step, prompt_arg};
use pmcp::server::workflow::{
    SequentialWorkflow, ToolHandle, WorkflowPromptHandler, WorkflowStep,
};
use pmcp::server::{PromptHandler, ResourceHandler, ToolHandler};
use pmcp::types::content::Role;
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::RequestHandlerExtra;
use pmcp_agent::{
    AgentEngine, CompletionError, CompletionSource, InMemoryStore, ResolvedAgentConfig,
    RunOutcome, ToolCall, ToolCallResult, ToolInvoker,
};
use serde_json::{json, Value};

const RULE: &str = "──────────────────────────────────────────────────────────";

const REFUNDS_SKILL_MD: &str = "---\nname: refunds\ndescription: Process customer refund requests per company policy\n---\n# Refund Processing\n\n1. Call `get_order` with the order id. Save the result.\n2. Call `check_eligibility` with the order. Save the result.\n3. Judgment: if `eligible` is false, do NOT refund — reply with a decline citing the reason. Otherwise call `issue_refund` with the order id and total.\n";

// ── Shared domain tools (with an execution log = forensic layer) ──────

#[derive(Default, Clone)]
struct ExecLog(Arc<Mutex<Vec<String>>>);
impl ExecLog {
    fn record(&self, tool: &str, args: &Value) {
        self.0.lock().unwrap().push(format!("{tool}({args})"));
    }
    fn entries(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
    fn executed(&self, tool: &str) -> bool {
        self.entries().iter().any(|e| e.starts_with(tool))
    }
}

fn run_tool(log: &ExecLog, name: &str, args: &Value) -> Value {
    log.record(name, args);
    match name {
        "get_order" => {
            let id = args.get("id").and_then(Value::as_str).unwrap_or("?");
            let status = if id == "ORD-BAD" { "returned" } else { "delivered" };
            json!({ "id": id, "total": 42.50, "status": status })
        },
        "check_eligibility" => {
            let status = args
                .pointer("/order/status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            json!({ "eligible": status == "delivered", "reason": "returned orders are not refundable" })
        },
        "issue_refund" => json!({ "refund_id": "R-1001", "refunded": args.get("amount").cloned().unwrap_or(Value::Null) }),
        other => json!({ "error": format!("unknown tool {other}") }),
    }
}

/// ToolHandler adapter over `run_tool` (for the workflow prompt handler).
struct DomainTool {
    name: &'static str,
    log: ExecLog,
}
#[async_trait]
impl ToolHandler for DomainTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(run_tool(&self.log, self.name, &args))
    }
}

// ── The shared policy brain (used by surfaces A and C verbatim) ───────

enum Action {
    CallTool { name: &'static str, args: Value },
    Finish { summary: String },
}

/// The LLM's decision rules, as one pure function over observations.
/// Surface A hosts it client-side; surface C hosts it inside the remote
/// agent loop. SAME code, different locus — that is the entire point.
fn decide(
    order_id: &str,
    order: Option<&Value>,
    eligibility: Option<&Value>,
    refund: Option<&Value>,
) -> Action {
    match (order, eligibility, refund) {
        (None, _, _) => Action::CallTool {
            name: "get_order",
            args: json!({ "id": order_id }),
        },
        (Some(o), None, _) => Action::CallTool {
            name: "check_eligibility",
            args: json!({ "order": o }),
        },
        (Some(o), Some(e), None) if e["eligible"] == json!(true) => Action::CallTool {
            name: "issue_refund",
            args: json!({ "id": o["id"], "amount": o["total"] }),
        },
        (_, Some(e), None) => Action::Finish {
            summary: format!("Declined: {}", e["reason"]),
        },
        (_, _, Some(r)) => Action::Finish {
            summary: format!("Refund {} issued", r["refund_id"]),
        },
    }
}

// ── Per-surface metrics ───────────────────────────────────────────────

#[derive(Debug, Default)]
struct Metrics {
    client_requests: usize,
    client_llm_turns: usize,
    bytes_into_client: usize,
    refund_issued: bool,
    outcome: String,
}

// ── Surface A: SKILL — client LLM executes the procedure ──────────────

async fn surface_a_skill(order_id: &str) -> anyhow::Result<Metrics> {
    let log = ExecLog::default();
    let skills = pmcp::server::skills::Skills::new()
        .add(pmcp::server::skills::Skill::new("refunds", REFUNDS_SKILL_MD))
        .into_handler()?;

    let mut m = Metrics::default();

    // Client fetches the skill (1 round-trip; skill text enters client context)
    let read = skills
        .read("skill://refunds/SKILL.md", RequestHandlerExtra::default())
        .await?;
    m.client_requests += 1;
    m.bytes_into_client += serde_json::to_string(&read)?.len();
    m.client_llm_turns += 1; // reads skill, plans

    // Client LLM executes the procedure: each tool call is a round-trip and
    // each result feeds another LLM turn.
    let (mut order, mut eligibility, mut refund) = (None, None, None);
    let outcome = loop {
        match decide(order_id, order.as_ref(), eligibility.as_ref(), refund.as_ref()) {
            Action::CallTool { name, args } => {
                let result = run_tool(&log, name, &args); // = tools/call round-trip
                m.client_requests += 1;
                m.bytes_into_client += result.to_string().len();
                m.client_llm_turns += 1;
                match name {
                    "get_order" => order = Some(result),
                    "check_eligibility" => eligibility = Some(result),
                    _ => refund = Some(result),
                }
            },
            Action::Finish { summary } => break summary,
        }
    };
    m.refund_issued = log.executed("issue_refund");
    m.outcome = outcome;
    Ok(m)
}

// ── Surface B: WORKFLOW PROMPT — server pre-executes ──────────────────

async fn surface_b_workflow(order_id: &str) -> anyhow::Result<Metrics> {
    let log = ExecLog::default();
    let mut infos = HashMap::new();
    let mut handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();
    for name in ["get_order", "check_eligibility", "issue_refund"] {
        infos.insert(
            Arc::from(name),
            pmcp::server::workflow::ToolInfo {
                name: name.to_string(),
                description: format!("{name} (domain tool)"),
                input_schema: json!({ "type": "object" }),
            },
        );
        handlers.insert(
            Arc::from(name),
            Arc::new(DomainTool { name: match name {
                "get_order" => "get_order",
                "check_eligibility" => "check_eligibility",
                _ => "issue_refund",
            }, log: log.clone() }) as Arc<dyn ToolHandler>,
        );
    }

    let wf = SequentialWorkflow::new("refund_flow", "Process a refund per policy")
        .argument("order_id", "The order to refund", true)
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
        .step(
            WorkflowStep::new("issue_refund", ToolHandle::new("issue_refund"))
                .arg("id", field("order", "id"))
                .arg("amount", field("order", "total"))
                .bind("refund")
                .with_guidance("Only issue if eligibility.eligible is true."),
        );
    wf.validate().map_err(|e| anyhow::anyhow!("{e}"))?;
    let ph = WorkflowPromptHandler::new(wf, infos, handlers, None::<Arc<dyn ResourceHandler>>);

    let mut m = Metrics::default();
    let mut args = HashMap::new();
    args.insert("order_id".to_string(), order_id.to_string());
    let result = ph.handle(args, RequestHandlerExtra::default()).await?; // = prompts/get
    m.client_requests += 1;
    m.bytes_into_client += serde_json::to_string(&result)?.len();
    m.client_llm_turns += 1; // one turn to continue from the transcript
    m.refund_issued = log.executed("issue_refund");
    m.outcome = format!(
        "{} messages returned; server executed: [{}]",
        result.messages.len(),
        log.entries()
            .iter()
            .map(|e| e.split('(').next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(m)
}

// ── Surface C: AGENT — delegated loop hosts the same policy brain ─────

struct RuleSource {
    order_id: String,
    observations: Arc<Mutex<(Option<Value>, Option<Value>, Option<Value>)>>,
    turns: Arc<AtomicUsize>,
}
#[async_trait]
impl CompletionSource for RuleSource {
    async fn create_message(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        self.turns.fetch_add(1, Ordering::SeqCst);
        let (order, eligibility, refund) = self.observations.lock().unwrap().clone();
        // The remote "LLM" runs the SAME decide() as surface A's client LLM.
        Ok(
            match decide(&self.order_id, order.as_ref(), eligibility.as_ref(), refund.as_ref()) {
                Action::CallTool { name, args } => CreateMessageResultWithTools::new(
                    "rule-model",
                    Role::Assistant,
                    vec![SamplingMessageContent::ToolUse {
                        name: name.into(),
                        id: format!("call-{name}"),
                        input: args,
                        meta: None,
                    }],
                )
                .with_stop_reason("tool_use"),
                Action::Finish { summary } => CreateMessageResultWithTools::new(
                    "rule-model",
                    Role::Assistant,
                    vec![SamplingMessageContent::Text {
                        text: summary,
                        meta: None,
                    }],
                )
                .with_stop_reason("end_turn"),
            },
        )
    }
}

struct RecordingInvoker {
    log: ExecLog,
    observations: Arc<Mutex<(Option<Value>, Option<Value>, Option<Value>)>>,
}
#[async_trait]
impl ToolInvoker for RecordingInvoker {
    async fn invoke(&self, call: ToolCall) -> ToolCallResult {
        let result = run_tool(&self.log, &call.name, &call.arguments);
        let mut obs = self.observations.lock().unwrap();
        match call.name.as_str() {
            "get_order" => obs.0 = Some(result.clone()),
            "check_eligibility" => obs.1 = Some(result.clone()),
            _ => obs.2 = Some(result.clone()),
        }
        ToolCallResult::ok(call.id, result)
    }
}

async fn surface_c_agent(order_id: &str) -> anyhow::Result<Metrics> {
    let log = ExecLog::default();
    let observations = Arc::new(Mutex::new((None, None, None)));
    let turns = Arc::new(AtomicUsize::new(0));
    let config = ResolvedAgentConfig::new(
        "You are the refunds agent. Follow the refunds skill.",
        "rule-model",
        4096,
        8,
    );
    let engine = AgentEngine::new(
        RuleSource {
            order_id: order_id.to_string(),
            observations: Arc::clone(&observations),
            turns: Arc::clone(&turns),
        },
        RecordingInvoker {
            log: log.clone(),
            observations: Arc::clone(&observations),
        },
        InMemoryStore::default(),
        config,
    );

    let mut m = Metrics::default();
    let outcome = engine.run(&format!("run-{order_id}")).await; // = ONE tools/call from the client
    m.client_requests += 1;
    m.client_llm_turns = 0; // the client's LLM does nothing
    let final_text = match &outcome {
        RunOutcome::Completed { result } => serde_json::to_string(&result.assistant_message)?,
        other => format!("{other:?}"),
    };
    m.bytes_into_client += final_text.len();
    m.refund_issued = log.executed("issue_refund");
    m.outcome = format!(
        "{final_text} (remote llm turns: {}, remote tool calls: {})",
        turns.load(Ordering::SeqCst),
        log.entries().len()
    );
    Ok(m)
}

// ── Reporting ─────────────────────────────────────────────────────────

fn print_scenario(title: &str, a: &Metrics, b: &Metrics, c: &Metrics) {
    println!("\n{RULE}");
    println!("Scenario: {title}");
    println!("{RULE}");
    println!("  {:<28} {:>10} {:>12} {:>10}", "", "A: skill", "B: workflow", "C: agent");
    println!(
        "  {:<28} {:>10} {:>12} {:>10}",
        "client round-trips", a.client_requests, b.client_requests, c.client_requests
    );
    println!(
        "  {:<28} {:>10} {:>12} {:>10}",
        "client LLM turns", a.client_llm_turns, b.client_llm_turns, c.client_llm_turns
    );
    println!(
        "  {:<28} {:>10} {:>12} {:>10}",
        "bytes into client context", a.bytes_into_client, b.bytes_into_client, c.bytes_into_client
    );
    println!(
        "  {:<28} {:>10} {:>12} {:>10}",
        "refund issued?", a.refund_issued, b.refund_issued, c.refund_issued
    );
    println!("  outcomes:");
    println!("    A: {}", a.outcome);
    println!("    B: {}", b.outcome);
    println!("    C: {}", c.outcome);
}

fn print_verdict() {
    println!("\n{RULE}");
    println!("VERDICT — the measured decision matrix");
    println!("{RULE}");
    println!(
        r#"
The three mechanisms are one spectrum — WHERE EXECUTION AND JUDGMENT LIVE —
with skills as the shared content type:

                       A: skill          B: workflow prompt   C: agent
  judgment locus       client LLM,       server executes;     remote LLM,
                       pre-action        judgment post-hoc    pre-action
  client needs         resources/read    prompts support      tools only
                       (+skills host)
  context cost         624 B measured    1401 B measured      82 B measured
                       (scales with      (transcript carries  (final result
                       skill size)       the full ceremony)   only — always
                                                              cheapest)
  round-trips          most              one                  one
  trust surface        skill text into   server executes      everything
                       client context    with server creds    delegated

MEASURED, not asserted: in the ineligible scenario, surfaces A and C issue
NO refund (the shared decide() gates the side effect BEFORE it happens);
surface B issues the refund anyway — the deterministic prefix cannot host
judgment. That single row is the positioning answer:

  - SKILL: judgment must stay with the caller; tools may span servers.
  - WORKFLOW PROMPT: steps are deterministic and pre-execution saves
    round-trips — keep judgment-gated side effects OUT of the prefix.
  - AGENT: the caller should hold nothing — process, creds, and model
    all live remotely; cheapest context, most delegated trust.

And they COMPOSE rather than compete: a workflow projects to a skill
(spike 009), an agent consumes a pinned skill as instructions (spike 010),
and the same decide() ran at two loci in this very binary — the mechanism
choice is deployment, not content.
"#
    );
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("{RULE}");
    println!("Spike 011: one task, three mechanisms");
    println!("{RULE}");

    let (a, b, c) = (
        surface_a_skill("ORD-77").await?,
        surface_b_workflow("ORD-77").await?,
        surface_c_agent("ORD-77").await?,
    );
    // Eligible: everyone should refund.
    assert!(a.refund_issued && b.refund_issued && c.refund_issued);
    assert_eq!(a.client_llm_turns, 4);
    assert_eq!(b.client_requests, 1);
    assert_eq!(c.client_llm_turns, 0);
    print_scenario("ELIGIBLE order ORD-77 — all three should refund", &a, &b, &c);

    let (a2, b2, c2) = (
        surface_a_skill("ORD-BAD").await?,
        surface_b_workflow("ORD-BAD").await?,
        surface_c_agent("ORD-BAD").await?,
    );
    // Ineligible: A and C must NOT refund; B (as modeled) does anyway.
    assert!(!a2.refund_issued, "skill surface gates the side effect");
    assert!(!c2.refund_issued, "agent surface gates the side effect");
    assert!(
        b2.refund_issued,
        "workflow surface executes the prefix mechanically — the measured defect"
    );
    print_scenario(
        "INELIGIBLE order ORD-BAD — only judgment-bearing surfaces decline",
        &a2, &b2, &c2,
    );
    println!("\n  ❗ MEASURED: surface B issued refund R-1001 for a returned order.");
    println!("     Same tools, same data — the only variable is judgment locus.");

    print_verdict();
    Ok(())
}
