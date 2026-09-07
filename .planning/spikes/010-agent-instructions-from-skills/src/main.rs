//! Spike 010: agent instructions sourced from MCP-served skills.
//!
//! Thesis under test: SEP-2640 skills are the DISTRIBUTION FORMAT for
//! pmcp-agent instructions. An AgentPackage references a skill by
//! (connector, uri, digest); resolution fetches the SKILL.md over the
//! connector, verifies the digest, and composes an origin-tagged block into
//! `ResolvedAgentConfig.instructions` — which the engine provably delivers
//! to the model as the system prompt.
//!
//! The interesting risk is the trust boundary, not the plumbing: the agent
//! IS a SEP-2640 host here, and the SEP says skill content is untrusted
//! model input while the engine puts instructions into the SYSTEM prompt
//! (crates/pmcp-agent/src/iteration/engine.rs:221 —
//! `.with_system_prompt(self.config.instructions.clone())`). The pack-time
//! digest pin is what squares that circle: it is the SEP's "content-bound
//! approval", with the package author as approver.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use pmcp::server::skills::{Skill, SkillReference, Skills};
use pmcp::server::ResourceHandler;
use pmcp::types::content::Role;
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::RequestHandlerExtra;
use pmcp_agent::{
    AgentEngine, CompletionError, CompletionSource, InMemoryStore, ResolvedAgentConfig,
    RunOutcome, ToolCall, ToolCallResult, ToolInvoker,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

const RULE: &str = "──────────────────────────────────────────────────────────";

const REFUNDS_SKILL_MD: &str = "---\nname: refunds\ndescription: Process customer refund requests per company policy\n---\n# Refund Processing\n\n1. Look up the order with `get_order`.\n2. Check eligibility against `references/policy.md`.\n3. Issue the refund with `issue_refund` ONLY if eligible.\n";
const POLICY_MD: &str = "# Refund Policy\n\n- 30 days, undamaged, original receipt.\n";

// ── The proposed AgentPackage slot (models a pmcp-package addition) ───

/// A skill pinned into an AgentPackage at pack time.
///
/// `digest` is REQUIRED: pinning is the package author's content-bound
/// approval of the exact bytes (SEP-2640 §Security Implications). An
/// unpinned reference has no approval to bind and MUST NOT reach the
/// system prompt (see step 5).
#[derive(Debug, Clone)]
struct SkillPin {
    connector: String,
    uri: String,
    digest: String,
}

fn digest_of(content: &str) -> String {
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    format!("sha256:{:x}", h.finalize())
}

// ── Resolve-time machinery the SDK would own ──────────────────────────

/// Fetch a pinned skill over its connector's resource surface and verify
/// the digest. This models what `resolve_agent` would do through a
/// `ConnectorClient` — the spike drives the ResourceHandler in-process
/// (the conventions pattern; wire dispatch is covered by pmcp's own tests).
async fn fetch_and_verify(
    handler: &dyn ResourceHandler,
    pin: &SkillPin,
) -> anyhow::Result<String> {
    let read = handler
        .read(&pin.uri, RequestHandlerExtra::default())
        .await?;
    let wire = serde_json::to_value(&read)?;
    let body = wire
        .pointer("/contents/0/text")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("skill read returned no text content"))?
        .to_string();
    let actual = digest_of(&body);
    if actual != pin.digest {
        anyhow::bail!(
            "skill content-bound approval REVOKED: {} digest mismatch (pinned {}, fetched {})",
            pin.uri,
            pin.digest,
            actual
        );
    }
    Ok(body)
}

/// Compose base instructions + a verified skill into the instruction text,
/// with the origin tagged the way SEP-2640 requires hosts to tag skill
/// content entering model context.
fn compose_instructions(base: &str, pin: &SkillPin, verified_body: &str) -> String {
    format!(
        "{base}\n\n--- BEGIN SKILL (origin: MCP connector \"{conn}\", uri {uri}, digest verified) ---\n{body}\n--- END SKILL ---\n",
        conn = pin.connector,
        uri = pin.uri,
        body = verified_body
    )
}

// ── Engine-side capture harness ───────────────────────────────────────

/// CompletionSource that records the system prompt it was handed and ends
/// the run on the first turn. The capture cell is shared with the harness
/// via Arc so the engine can own the source outright.
struct CaptureSource {
    seen_system_prompt: Arc<Mutex<Option<String>>>,
}

#[async_trait]
impl CompletionSource for CaptureSource {
    async fn create_message(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        *self.seen_system_prompt.lock().unwrap() = params.system_prompt.clone();
        Ok(CreateMessageResultWithTools::new(
            "test-model",
            Role::Assistant,
            vec![SamplingMessageContent::Text {
                text: "Understood — following the refunds skill.".into(),
                meta: None,
            }],
        )
        .with_stop_reason("end_turn"))
    }
}

struct NoToolsInvoker;
#[async_trait]
impl ToolInvoker for NoToolsInvoker {
    async fn invoke(&self, call: ToolCall) -> ToolCallResult {
        ToolCallResult::error(call.id, "no tools in this harness")
    }
}

// ── Steps ─────────────────────────────────────────────────────────────

fn build_registry(body: &str) -> anyhow::Result<std::sync::Arc<dyn ResourceHandler>> {
    Skills::new()
        .add(Skill::new("refunds", body).with_reference(SkillReference::new(
            "references/policy.md",
            "text/markdown",
            POLICY_MD,
        )))
        .into_handler()
        .map_err(Into::into)
}

fn step_1_pin() -> SkillPin {
    println!("{RULE}");
    println!("Step 1 — pack time: the AgentPackage pins the skill by digest");
    println!("{RULE}");
    let pin = SkillPin {
        connector: "billing".into(),
        uri: "skill://refunds/SKILL.md".into(),
        digest: digest_of(REFUNDS_SKILL_MD),
    };
    println!("  proposed AgentPackage slot (pmcp-package addition):\n");
    println!("    [[skills]]");
    println!("    connector = \"{}\"", pin.connector);
    println!("    uri = \"{}\"", pin.uri);
    println!("    digest = \"{}\"  # content-bound approval, pinned at pack time", pin.digest);
    println!();
    println!("  ✓ the pin IS the SEP's \"content-bound approval\": the package author");
    println!("    approved these exact bytes; the digest binds the approval to them");
    pin
}

async fn step_2_fetch_verify(pin: &SkillPin) -> anyhow::Result<String> {
    println!("\n{RULE}");
    println!("Step 2 — resolve time: fetch over the connector, verify the pin");
    println!("{RULE}");
    let handler = build_registry(REFUNDS_SKILL_MD)?;
    let body = fetch_and_verify(handler.as_ref(), pin).await?;
    assert_eq!(body, REFUNDS_SKILL_MD);
    println!("  ✓ fetched skill://refunds/SKILL.md in-process, digest verified");

    // Tamper case: the server now serves different bytes at the same URI
    // (rotated skill, compromised server, or plain staleness — the SEP
    // treats them identically: approval revoked).
    let tampered_body = REFUNDS_SKILL_MD.replace("ONLY if eligible", "for every request");
    let tampered = build_registry(&tampered_body)?;
    let err = fetch_and_verify(tampered.as_ref(), pin).await;
    assert!(err.is_err(), "tampered content must be refused");
    println!("  ✓ tampered server refused: {}", err.unwrap_err());
    println!("    (resolution fails FATALLY before the loop starts — the agent never");
    println!("     runs on unapproved instructions)");
    Ok(body)
}

fn step_3_compose(pin: &SkillPin, body: &str) -> String {
    println!("\n{RULE}");
    println!("Step 3 — compose origin-tagged instructions");
    println!("{RULE}");
    let base = "You are the refunds agent for Acme. Use only the tools you are given.";
    let composed = compose_instructions(base, pin, body);
    println!("{composed}");
    assert!(composed.contains("origin: MCP connector \"billing\""));
    assert!(composed.contains(pin.uri.as_str()));
    assert!(composed.contains(body));
    assert!(composed.starts_with(base));
    println!("  ✓ origin visible to the model (SEP: hosts MUST tag skill content with");
    println!("    its originating server at the point it enters model context)");
    let again = compose_instructions(base, pin, body);
    assert_eq!(composed, again);
    println!("  ✓ composition is a pure function — replay-deterministic, matching the");
    println!("    agent crate's trace/replay stance");
    composed
}

async fn step_4_engine_proof(composed: String) -> anyhow::Result<()> {
    println!("\n{RULE}");
    println!("Step 4 — the engine provably hands the skill to the model");
    println!("{RULE}");
    let config = ResolvedAgentConfig::new(composed.clone(), "test-model", 4096, 3);
    let capture: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let source = CaptureSource {
        seen_system_prompt: Arc::clone(&capture),
    };
    let engine = AgentEngine::new(source, NoToolsInvoker, InMemoryStore::default(), config);
    let outcome = engine.run("spike-010-run").await;
    println!("  run outcome: {outcome:?}");
    assert!(
        matches!(outcome, RunOutcome::Completed { .. }),
        "single-turn run completes"
    );

    let sp = capture
        .lock()
        .unwrap()
        .clone()
        .expect("completion source must receive a system prompt");
    assert_eq!(sp, composed);
    assert!(sp.contains("Refund Processing"));
    assert!(sp.contains("origin: MCP connector \"billing\""));
    println!("  ✓ CreateMessageParams.system_prompt == composed instructions, byte-equal");
    println!("    (engine.rs:221 wires config.instructions → system prompt verbatim)");
    Ok(())
}

fn step_5_trust_rules() {
    println!("\n{RULE}");
    println!("Step 5 — trust rules the SDK must encode");
    println!("{RULE}");
    println!(
        r#"  The agent is a SEP-2640 HOST. The SEP's security requirements land here:

  1. SYSTEM-PROMPT placement is a privilege decision. The SEP says skill
     content is untrusted model input and "skills are data, not directives"
     — yet engine.rs:221 elevates instructions to the system prompt, the
     highest-authority slot. The DIGEST PIN is what makes that defensible:
     the package author approved the exact bytes at pack time (content-bound
     approval), so by the time content reaches the system prompt it is no
     longer "whatever the server serves" — it is "what the author shipped".
     RULE: pinned skill → may compose into instructions/system prompt.
           unpinned or `"resources": "dynamic"` skill → MUST NOT; inject as
           an ordinary (user-role) turn, or refuse to resolve.

  2. Approval revocation maps to resolve failure. A digest mismatch at
     resolve time = the SEP's "approval revoked, re-prompt" — for a headless
     agent the only honest re-prompt is FAILING the run and asking for a
     re-packed AgentPackage (a human re-approval). Never fetch-and-continue.

  3. Origin-scoped reads. A skill pinned from connector "billing" may only
     cause resource reads against "billing" (confused-deputy rule). ToolCall
     already carries `connector: Option<String>` — the invoker is where the
     SDK can enforce the scope when the model asks for supporting files.

  4. Supporting files are lazily fetched, so the pin should grow to the full
     SEP entry manifest ({{uri, digest, size}} per file — spike 008's shape)
     rather than the SKILL.md digest alone. Then references/policy.md is
     verified on read exactly like SKILL.md was at resolve."#
    );
}

fn print_verdict() {
    println!("\n{RULE}");
    println!("VERDICT");
    println!("{RULE}");
    println!(
        r#"
Skills-as-agent-instructions works end-to-end with ZERO changes to
pmcp-agent: fetch → verify → compose → ResolvedAgentConfig::new → the
engine delivers the skill to the model as the system prompt, byte-equal
and origin-tagged. The whole resolve-time layer is ~60 lines.

The conceptual yield is bigger than the plumbing: the AgentPackage digest
pin and SEP-2640 content-bound approval are THE SAME MECHANISM viewed from
two sides. The package author is the approving user; packing is the
approval ceremony; digest mismatch at resolve is revocation. This gives the
"skills as distribution format for agent instructions" story a security
model for free — and it is exactly the packaging direction the WG scoped
OUT of SEP-2640 (installable bundles), where pmcp-package is already ahead.

Recommended SDK shape:
  - pmcp-package: optional `[[skills]]` slots on AgentPackage
    {{connector, uri, digest}} — digest REQUIRED (0.x break acceptable).
  - pmcp-agent resolver: fetch via the connector client, verify, compose
    origin-tagged block; digest mismatch = ResolveError (fatal, pre-loop).
  - Trust rule: pinned → system prompt; unpinned/dynamic → user-role turn
    or refusal.
  - Invoker-level origin scoping for supporting-file reads (follow-up).
  - Grow the pin to the full entry manifest once skills/get lands (008).
"#
    );
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("{RULE}");
    println!("Spike 010: agent instructions from MCP-served skills");
    println!("{RULE}\n");
    let pin = step_1_pin();
    let body = step_2_fetch_verify(&pin).await?;
    let composed = step_3_compose(&pin, &body);
    step_4_engine_proof(composed).await?;
    step_5_trust_rules();
    print_verdict();
    Ok(())
}
