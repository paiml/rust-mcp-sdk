---
spike: 011
idea: skills-positioning
name: three-mechanisms-one-task
type: standard
validates: "Given one domain task (refund flow), when implemented as skill vs workflow-prompt vs agent with the same tools and decision rules, then a harness measures round-trips, context weight, and where judgment ran — producing a decision matrix for SDK docs."
verdict: VALIDATED
related: [009-workflow-skill-projection, 010-agent-instructions-from-skills]
tags: [skills, workflow, pmcp-agent, positioning, decision-matrix]
---

# Spike 011: One Task, Three Mechanisms

## What This Validates

Given one refund task, when implemented as (A) a SEP-2640 skill the client
LLM executes, (B) a workflow prompt the server pre-executes, and (C) a
delegated pmcp-agent — same domain tools, same decision rules — then the
harness measures client round-trips, client LLM turns, bytes entering the
client's context, and whether a refund gets issued when it should not.

## Research

None external — this spike assembles parts proven by 009 (workflow prompt
handler in-process) and 010 (agent engine harness). The design trick is the
shared **policy brain**: one pure `decide(observations) -> Action` function
hosted client-side in surface A and inside the remote loop's
`CompletionSource` in surface C — making "where judgment runs" literal
shared code rather than a metaphor.

## How to Run

```bash
cargo run --manifest-path .planning/spikes/011-three-mechanisms-one-task/Cargo.toml
```

## What to Expect

Two scenarios × three surfaces, each with a measured metrics table, then
the decision-matrix verdict. All assertions pass; exit 0.

## Observability

Every surface shares one forensic layer: `ExecLog` records each domain-tool
execution (`tool(args)`), and the metrics struct counts client requests,
client LLM turns, and bytes entering client context. Surface C additionally
counts remote LLM turns and remote tool calls.

## Investigation Trail

1. Built the shared domain (`run_tool`: get_order / check_eligibility /
   issue_refund; `ORD-BAD` returns a returned-status order) and the shared
   `decide()` policy brain.
2. Surface A: client fetches SKILL.md via the shipped `Skills` handler,
   then loops `decide()` against direct tool calls (each = a round-trip).
   Surface B: 009's workflow via `WorkflowPromptHandler::handle`. Surface
   C: `AgentEngine` with a `RuleSource` completion source that runs the
   same `decide()` remotely, and a `RecordingInvoker` sharing observations.
3. First run: all assertions passed. The measurement then corrected the
   spike's own prior: the verdict table initially claimed context cost
   "highest for skill, middle for workflow" — measured, the WORKFLOW
   transcript is the heaviest at this scale (1401 B vs skill 624 B vs
   agent 82 B) because the transcript carries the full ceremony (plan +
   calls + results). The table was rewritten to carry the measured numbers.

## Results

**✓ VALIDATED.** Measured matrix (eligible scenario / ineligible scenario):

| metric | A: skill | B: workflow | C: agent |
|---|---|---|---|
| client round-trips | 4 / 3 | 1 / 1 | 1 / 1 |
| client LLM turns | 4 / 3 | 1 / 1 | 0 / 0 |
| bytes into client context | 624 / 587 | 1401 / 1405 | 82 / 106 |
| refund issued (eligible) | ✓ | ✓ | ✓ |
| **refund issued (INELIGIBLE)** | **no ✓** | **YES ❗** | **no ✓** |

**The headline row is the last one — measured, not asserted:** with
identical tools and data, surface B issued refund R-1001 for a returned
order, because the deterministic prefix cannot host judgment; A and C both
declined because `decide()` gates the side effect *before* it happens. The
only variable was judgment locus.

**Positioning guidance this produces (for SDK docs):**
- **Skill** — when judgment must stay with the caller's LLM; tools may span
  servers; costs the most round-trips.
- **Workflow prompt** — when steps are deterministic and pre-execution
  saves round-trips; keep judgment-gated side effects OUT of the prefix
  (or model them as elicitation/stop points).
- **Agent** — when the caller should hold nothing: process, creds, and
  model all remote; always the cheapest client context (82 B), most
  delegated trust.
- **Context cost is not monotone**: workflow transcripts can outweigh skill
  bodies at small scale; only the agent is unconditionally cheapest.

**They compose rather than compete**: workflow projects to skill (009),
agent consumes pinned skill (010), and the same `decide()` ran at two loci
in this binary — mechanism choice is deployment, not content.
