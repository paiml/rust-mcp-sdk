---
spike: 009
idea: skills-positioning
name: workflow-skill-projection
type: standard
validates: "Given a SequentialWorkflow, when projected via a skills-gated adapter, then a derived SKILL.md accurately describes steps/tools/guidance, is discoverable per SEP-2640, and prompt execution is unchanged."
verdict: VALIDATED
related: [002-skill-ergonomics-pragmatic, 008-sep-2640-drift-check]
tags: [skills, workflow, projection, tri-surface, positioning]
---

# Spike 009: Workflow → Skill Projection

## What This Validates

Given a `SequentialWorkflow`, when projected, then a derived SKILL.md
accurately describes its steps/tools/guidance, is discoverable through the
shipped `Skills` registry, and the workflow's `prompts/get` execution is
unchanged. Kill-risk under test: a mechanically derived SKILL.md could be
lossy garbage, which would sink the "one artifact, multiple surfaces" thesis.

## Research

- Workflow introspection surface (`SequentialWorkflow::{name, description,
  arguments, steps, instructions}`, `WorkflowStep::{name, tool, arguments,
  binding, guidance}`, `DataSource::{PromptArg, StepOutput, Constant}`) is
  fully public — the projection needs no SDK changes to read everything.
- `WorkflowPromptHandler::new(workflow, tools, tool_handlers, resource_handler)`
  is public, so the prompt surface is testable in-process via
  `PromptHandler::handle` (the conventions pattern; no wire needed).
- agentskills.io naming: skill `name` must match the directory (URI final
  segment) — lowercase alnum + hyphens. Workflow names (`refund_flow`) are
  NOT automatically legal; the projection owns slugification.

## How to Run

```bash
cargo run --manifest-path .planning/spikes/009-workflow-skill-projection/Cargo.toml
```

## What to Expect

Four steps: (1) prints the derived SKILL.md for a 3-step refund workflow;
(2) coverage + determinism assertions; (3) the derived skill flowing through
the shipped `Skills` registry; (4) the workflow prompt executing in-process
with its 9-message transcript, plus the surface-equivalence assertions.
All assertions pass; exit 0.

## Investigation Trail

1. Built `project_skill()` (~90 lines): frontmatter from workflow
   name/description, Context from workflow-level instructions, Inputs from
   argument specs, Procedure from steps (tool + rendered `DataSource`
   argument mappings + `Save the result as`), guidance as "Judgment:" lines,
   and a closing "Server-accelerated alternative" section cross-referencing
   the prompt *without redirecting* (the manual procedure stays complete —
   the dual-surface rule's spirit extended to a third surface).
2. First run compiled and passed every assertion. Two iterations after:
   fixed Context rendering (was Debug-formatting `PromptContent::Text`),
   and added the guidance-vs-bare comparison (1425 vs 1268 bytes).
3. The experiential payoff came from step 4's transcript: message [6] is the
   guidance ("Only issue the refund if `eligibility.eligible` is true…") —
   and messages [7]-[8] show the server had **already executed**
   `issue_refund` (R-1001 issued). The guidance is present in the transcript
   but cannot *gate* server-side execution: the LLM reading the prompt
   result is asked to make a judgment that is already moot. Post-hoc
   judgment is the workflow surface's blind spot, and the skill surface's
   reason to exist.

## Results

**✓ VALIDATED.** Projection is real, cheap, and faithful:

- **Coverage**: every fact the workflow holds (steps, tools, data-flow
  argument mappings, bindings, guidance, instructions, argument specs) lands
  in the derived text — asserted string-by-string.
- **Determinism**: re-derivation is byte-equal.
- **Discoverability**: the derived skill flows through the shipped `Skills`
  registry untouched (`skill://refund-flow/SKILL.md`, byte-identical read),
  and `as_prompt_text() == body`, so the fallback-prompt invariant holds for
  free.
- **Unchanged execution**: the same workflow still executes via
  `WorkflowPromptHandler` (9-message transcript, partial results inline).
- **Surface equivalence**: both surfaces name exactly the same three tools.

**Thesis refinement (the load-bearing finding):** the workflow and skill
surfaces are not two transports for the same behavior — they differ on
**where judgment runs**. Server-side execution runs every deterministic step
regardless of guidance prose; the skill surface exists precisely to put the
guidance *before* the actions. The projection makes that line legible, and
its value is highest for workflows with meaningful `with_guidance` text
(the quality dial: mechanical skeleton is valid but thin).

**SDK shape recommended:**
1. `SequentialWorkflow::as_skill()` (or `Skills::from_workflow(&wf)`) —
   renderer owned by the SDK so surfaces cannot drift.
2. Slugification rule owned by the projection (`refund_flow` → `refund-flow`).
3. Generated "Server-accelerated alternative" section cross-references the
   prompt, never redirects.
4. DX guardrail: a side-effecting step whose guidance implies a gate should
   trip a projection-time warning — guidance cannot gate server-side
   execution (the post-hoc judgment trap, observed live in step 4).
