# Skills Positioning: Tri-Surface (Skill / Workflow / Agent)

The SDK carries three instruction-carrying mechanisms: SEP-2640 Skills, prompts-
as-workflows (`SequentialWorkflow`), and pmcp-agent. Spikes 009-011 settled how
they relate: **they are points on one execution-locus axis sharing one content
type — the axis is WHERE JUDGMENT RUNS, not transport.** Skill is the canonical
content primitive; the other two project to it (workflow → skill) or consume
from it (agent ← skill). Alongside, composing — never instead.

## Requirements

From idea `skills-positioning` (MANIFEST.md):

- **Workflow↔skill is a projection, not a rival feature.** Ship
  `SequentialWorkflow::as_skill()` (SDK-owned renderer, deterministic,
  byte-equal on re-derivation) so the two surfaces cannot drift. The generated
  body must stay a COMPLETE manual procedure; the "Server-accelerated
  alternative" section cross-references the workflow prompt but never redirects.
- **Projection owns name slugification.** Workflow names are not
  agentskills-legal (`refund_flow` → `refund-flow`); the final URI segment must
  equal the skill name.
- **Post-hoc judgment is the workflow surface's blind spot.** Server-side
  execution runs every deterministic step regardless of guidance prose.
  Projection-time warning required when a side-effecting step carries gate-like
  guidance.
- **AgentPackage digest pins ARE SEP-2640 content-bound approval.**
  `[[skills]]` slots `{connector, uri, digest}` with digest REQUIRED — the
  package author is the approving user, packing is the approval ceremony,
  resolve-time digest mismatch is revocation (fatal ResolveError, pre-loop;
  never fetch-and-continue). Composition must be origin-tagged.
- **System-prompt placement is a privilege decision.** Pinned skill → may
  compose into instructions/system prompt. Unpinned or `"resources": "dynamic"`
  → must NOT; user-role turn or refusal. Skill-driven supporting-file reads
  stay origin-scoped to the pinning connector (`ToolCall.connector` is the
  enforcement point).
- **Positioning is settled: alongside, composing — never instead.** Measured:
  same tools + same data, the ineligible order got refunded ONLY by the
  workflow surface. Context cost is NOT monotone.

## How to Build It

### 1. Workflow → Skill projection (spike 009, ~90 lines, zero SDK changes needed to read)

The workflow introspection surface is fully public: `SequentialWorkflow::{name,
description, arguments, steps, instructions}`, `WorkflowStep::{name, tool,
arguments, binding, guidance}`, `DataSource::{PromptArg, StepOutput, Constant}`.
The renderer maps:

- frontmatter ← workflow name (slugified) + description
- Context section ← workflow-level instructions (render `PromptContent::Text`
  properly — Debug-formatting it was spike 009's only real bug)
- Inputs section ← argument specs
- Procedure ← steps: tool + rendered `DataSource` argument mappings +
  "Save the result as {binding}"
- "Judgment:" lines ← `with_guidance` text
- Closing "Server-accelerated alternative" section ← cross-references the
  prompt name, manual procedure stays complete

Proven properties to keep as tests: full coverage (every workflow fact appears
in the derived text, asserted string-by-string), determinism (re-derivation
byte-equal), registry pass-through (`skill://refund-flow/SKILL.md` byte-
identical read), `as_prompt_text() == body` (dual-surface invariant holds for
free), surface equivalence (both surfaces name exactly the same tools).

SDK shape: `SequentialWorkflow::as_skill()` or `Skills::from_workflow(&wf)` —
renderer owned by the SDK so surfaces cannot drift. Target the CURRENT
SEP-2640 entry shape (digests included) from day one.

### 2. Agent ← Skill resolution (spike 010, ~60 lines, zero pmcp-agent changes)

Layer lives in `pmcp-agent::config::resolver`:

1. **Pack time:** `SkillPin {connector, uri, digest}` — digest computed at
   pack time, REQUIRED. Proposed as optional `[[skills]]` slots on
   AgentPackage (0.x break acceptable per the audience philosophy).
2. **Resolve time:** `fetch_and_verify` (~25 lines) reads the pinned URI over
   the connector's resource surface, compares sha256 digests. Mismatch =
   fatal ResolveError before the loop starts ("approval REVOKED").
3. **Compose:** wrap the verified body in explicit delimiters after base
   instructions:
   `--- BEGIN SKILL (origin: MCP connector "billing", uri ..., digest verified) ---`
   Pure function, byte-equal on recomposition (replay-deterministic).
4. **Deliver:** `ResolvedAgentConfig.instructions` flows verbatim into
   `CreateMessageParams.system_prompt` (engine.rs:221) — spike 010 proved
   byte-equality with a `CaptureSource` sharing an `Arc<Mutex<Option<String>>>`
   cell (the engine owns its seams privately; don't try to read them back out).

Once `skills/get` lands (see sep-2640-conformance.md), grow the pin to the
full entry manifest (`{uri, digest, size}` per file) so lazily-fetched
supporting files verify exactly like SKILL.md.

### 3. Decision matrix for SDK docs (spike 011, measured — not asserted)

| metric | A: skill | B: workflow | C: agent |
|---|---|---|---|
| client round-trips (eligible/ineligible) | 4 / 3 | 1 / 1 | 1 / 1 |
| client LLM turns | 4 / 3 | 1 / 1 | 0 / 0 |
| bytes into client context | 624 / 587 | 1401 / 1405 | 82 / 106 |
| refund issued (eligible) | yes | yes | yes |
| refund issued (INELIGIBLE) | no | **YES — defect** | no |

Docs guidance this produces:

- **Skill** — judgment must stay with the caller's LLM; tools may span
  servers; costs the most round-trips.
- **Workflow prompt** — steps are deterministic and pre-execution saves
  round-trips; keep judgment-gated side effects OUT of the deterministic
  prefix (or model them as elicitation/stop points).
- **Agent** — the caller should hold nothing: process, creds, and model all
  remote; always the cheapest client context; most delegated trust.

## What to Avoid

- **Never fetch-and-continue on a digest mismatch.** For a headless agent the
  only honest "re-prompt" is failing the run and requiring a re-packed
  package. Mismatch = revocation, pre-loop, fatal.
- **Never compose an unpinned or `"resources": "dynamic"` skill into the
  system prompt.** Placement there is a privilege granted by the pack-time
  pin. Without it: user-role turn or refuse to resolve.
- **Never put a judgment-gated side effect inside a workflow's deterministic
  prefix.** Observed live (spike 009 step 4): the transcript's guidance
  message ("Only issue the refund if eligible...") was immediately followed by
  the already-executed `issue_refund` — guidance prose cannot gate server-side
  execution; the LLM is asked to judge something already moot. Spike 011 then
  measured the consequence: the ineligible order got refunded by this surface
  only. The projection should trip a warning when a side-effecting step
  carries gate-like guidance.
- **Never let the "Server-accelerated alternative" section redirect.** It
  cross-references the workflow prompt; the manual procedure stays complete
  (the dual-surface rule's spirit, extended to a third surface).
- **Don't assume context cost is monotone.** The workflow transcript outweighed
  the skill body at demo scale (1401 B vs 624 B) — the transcript carries the
  full ceremony (plan + calls + results). Only the agent is unconditionally
  cheapest (82 B).
- **Don't frame the three mechanisms as competing transports.** They compose:
  workflow projects to skill (009), agent consumes pinned skill (010), and the
  same `decide()` function ran at two loci in one binary (011). Mechanism
  choice is deployment, not content.

## Constraints

- **Trust rules the SDK must encode** (from the SEP's Security Implications,
  applied): skill content is untrusted model input; origin MUST be visible to
  the model (the delimiter block is that requirement realized in prompt text);
  approvals are content-bound to the digest set; skill-driven reads are
  origin-scoped — a skill pinned from connector "billing" may only cause reads
  against "billing" (`ToolCall.connector` is the enforcement point).
- **The WG scoped OUT installable bundles** (skills + servers + subagents +
  configuration as one artifact) — exactly what pmcp-package is. The packaging
  side of skills is PMCP's to define; pmcp-package is already ahead there.
- **agentskills.io naming:** skill `name` = lowercase alnum + hyphens, must
  equal the URI's final segment. Workflow names like `refund_flow` are NOT
  automatically legal; slugification is the projection's job.
- **Projection quality dial:** the derived skill's value is highest for
  workflows with meaningful `with_guidance` text; a mechanical skeleton is
  valid but thin.
- **Harness patterns for testing this territory** (also in
  `.planning/spikes/CONVENTIONS.md`): shared-cell capture for private seams;
  `AgentEngine::new(source, invoker, InMemoryStore::default(), config)` with
  `CreateMessageResultWithTools::new(...).with_stop_reason(...)`; the
  policy-brain pattern — ONE pure `decide(observations) -> Action` hosted at
  each locus so divergent outcomes are measurements, not opinions.

## Origin

Synthesized from spikes: 009, 010, 011
Source files available in: sources/009-workflow-skill-projection/,
sources/010-agent-instructions-from-skills/, sources/011-three-mechanisms-one-task/
