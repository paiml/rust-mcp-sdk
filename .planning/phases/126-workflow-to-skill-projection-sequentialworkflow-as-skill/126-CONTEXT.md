# Phase 126: Workflow to skill projection — `SequentialWorkflow::as_skill()` - Context

**Gathered:** 2026-09-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 126 makes a `SequentialWorkflow` and its projected skill the SAME content,
rendered twice, unable to drift — because the SDK owns the renderer. It derives a
SEP-2640-conforming `Skill` from the workflow's already-public introspection surface
(`SequentialWorkflow::{name, description, arguments, steps, instructions}`,
`WorkflowStep::{name, tool, arguments, binding, guidance, resources,
template_bindings}`, `DataSource::{PromptArg, StepOutput, Constant}`), targeting
Phase 125's CURRENT entry shape — verbatim frontmatter plus a complete
`{uri, digest: sha256, size}` manifest — from day one, so 125's conformance work is
inherited rather than re-litigated.

**In scope:** the renderer (`src/server/skills/projection.rs`); the
`SequentialWorkflow::as_skill()` entry point and the `SkillProjection` builder;
projection-owned slugification; the SC-6 gate warning driven by `ToolAnnotations`;
an opt-in path that makes the projected body message [0] of the workflow prompt,
replacing the separately-emitted instruction messages; a golden-pinned render; a new
`s45` example; and the fuzz / property / unit / doctest coverage the project's ALWAYS
requirements mandate.

**Out of scope (recorded, not dropped):** the tri-surface decision-matrix docs
(spike-findings implementation-order item 25 — its own phase); `[[skills]]` digest
pins on `AgentPackage` (item 24); making the prompt prepend default-on; a frozen
byte-level semver contract for the rendered markdown; a provenance frontmatter key.

**What this phase does NOT change:** the `skills` feature stays opt-in — absent from
both `default` and `full`, tested through the `make test-skills` leg (Phase 125 D-09).
Existing `prompts/get` transcripts stay byte-identical unless the new opt-in flag is
set.

</domain>

<decisions>
## Implementation Decisions

### API shape & gating

- **D-01:** **`SequentialWorkflow::as_skill()` plus a `SkillProjection` builder.**
  `as_skill()` lives behind `#[cfg(feature = "skills")]` on the ungated
  `SequentialWorkflow` and delegates to the builder; the builder is what carries
  options (tool annotations today, section overrides later). Keeps the roadmap's
  discoverable name and leaves room to grow inputs without a breaking signature
  change. — **Reversibility:** costly — adding builder options later is additive,
  but renaming or removing a public inherent method on a published 2.x type is a
  major-version event.

- **D-02:** **Fallibility splits along the two entry points.** `as_skill() -> Skill`
  never fails; the builder's `build() -> Result<Skill>` is where strict checks live.
  This mirrors the module's own precedent — `Skill::with_reference` /
  `try_with_reference` at `src/server/skills.rs:362` and `:393`. — **Reversibility:**
  one-way — moving `as_skill()` to `Result<Skill>` later breaks every call site on
  the published 2.x line.

- **D-03:** **Renderer lives in a new `src/server/skills/projection.rs`.** Split
  `skills.rs` (already 157 KB) into a module directory; the workflow module keeps
  only the thin delegating `as_skill()`. The renderer ships with the `skills`
  feature, so `make test-skills` covers it, and PMAT cognitive complexity stays
  contained in a fresh file. The canonical public path remains
  `pmcp::server::skills::*`. — **Reversibility:** reversible — internal file layout;
  no public path moves.

- **D-04:** **The prompt prepend lands this phase, opt-in.** A
  `WorkflowPromptHandler` opt-in (builder flag, gated on `feature = "skills"`) makes
  message [0] the projected body, **replacing** the separately-emitted
  `instructions()` messages so the workflow's system prompt does not appear twice in
  one transcript. Without the flag every existing transcript is byte-identical, so
  spike 009's "unchanged execution" property still holds for current users. Rationale:
  shipping the renderer without its consumer leaves the anti-drift claim theoretical —
  with the flag the two surfaces share a *string*, not merely a renderer. —
  **Reversibility:** costly — once the flag exists, callers depend on the transcript
  shape it produces.

- **D-04a (AMENDMENT, locked 2026-09-02 — supersedes D-04's "replacing" clause).**
  Research measured that D-04's stated referent **does not exist**:
  `SequentialWorkflow::instructions()` (`src/server/workflow/sequential.rs:225-227`)
  reaches no production code path — `grep -n "instructions"` over
  `prompt_handler.rs` and `task_prompt_handler.rs` returns **0 matches**, and
  `InternalPromptMessage::to_protocol` (`conversion.rs:116`) has callers only in its
  own `#[cfg(test)] mod tests` plus one rustdoc example. There are no
  "separately-emitted `instructions()` messages" to replace.

  The duplication hazard D-04 was written to prevent is nonetheless real, and lands on
  different messages. `WorkflowPromptHandler::handle` (`prompt_handler.rs:750-957`)
  emits `[0] create_user_intent` (pushed `:771`, built `:406-427`) and
  `[1] create_assistant_plan` (pushed `:774`, built `:429-461`); the projected body's
  `## Procedure` section duplicates `[1]`'s step-and-tool list.

  **Resolution (user decision):** with the flag ON, the handler **prepends the
  projected body as message [0] and suppresses `create_assistant_plan`**.
  `create_user_intent` is **kept** — its `Parameters:` block is the only place the
  caller's actual argument *values* appear, and the projected body carries argument
  *specs*, not values. Suppressing both (the rejected third option) would drop that
  information from the transcript entirely.

  Resulting flag-ON sequence: `[0]` projected body · `[1] create_user_intent` ·
  `[2..]` guidance / resources / tool-call / tool-result messages, unchanged.
  Flag OFF stays byte-identical to today under every option, so this amendment does
  not touch the "unchanged execution" property. — **Reversibility:** costly, same as
  D-04 — the flag-ON transcript shape becomes a thing callers depend on.

  **Second-order defect recorded, out of scope:**
  `SequentialWorkflow::instruction()` (`sequential.rs:168`) is a shipped public
  builder method whose value is silently dropped by every served surface, while
  `examples/s31_workflow_minimal.rs:127` prints instructions and thereby implies they
  are live. That is a latent SDK defect independent of Phase 126 — do not fix it here.

### The dual-surface text

- **D-05:** **One byte-identical body everywhere.** The same bytes are served at
  `skill://{name}/SKILL.md` and prepended as message [0]. One string, one digest,
  no variant to keep in sync — no suppressed-section render. — **Reversibility:**
  costly — the digest published in `skills/list` binds pinning consumers to these
  bytes (see D-14).

- **D-06:** **The closing pointer is a user-education instruction to the model, not a
  cross-reference for a reader.** MCP prompts are user-controlled by design: the host
  surfaces them for a person to select, and the model cannot invoke one. A section
  telling the model to "prefer the prompt" is therefore inert text. Its only useful
  job is closing the *discovery* gap for users who do not know the prompt workflow
  exists. So the body carries one line instructing the model to mention, **once, at
  the end**, that this server also offers the `{workflow}` prompt which runs these
  steps server-side. The line is conditioned on context the model can actually
  resolve — a prompt result carries executed step results, a cold skill read does not —
  so the same bytes read correctly in both positions. It names the prompt and never
  redirects, satisfying the locked tri-surface requirement more literally than the
  spike's "Server-accelerated alternative" wording did. — **Reversibility:**
  reversible as prose, but any edit changes the digest (D-14).

### Gate warning (SC-6)

- **D-07:** **Side-effect detection reads `ToolAnnotations` supplied to the builder.**
  `SkillProjection::new(&wf).with_tools(...)` consults the shipped
  `ToolAnnotations { read_only_hint, destructive_hint }` (`src/types/tools.rs:20`)
  rather than inventing semantics. Consequence to document: a bare `as_skill()` holds
  no tool map and therefore cannot warn — the warning is a builder-path capability.

- **D-08:** **Explicit signal warns; missing annotations get a distinct
  "unverifiable" note.** Warn when `read_only_hint == Some(false)` or
  `destructive_hint == Some(true)`. When a guidance-bearing step's tool carries no
  annotations at all, emit a *separate* one-line diagnostic saying the check could
  not be performed. Rejected: following MCP's literal defaults (absent
  `read_only_hint` means not read-only), which would fire on essentially every
  existing workflow and is how a real warning gets muted.

- **D-09:** **Any guidance on a side-effecting step trips the warning — no text
  analysis, no phrase list.** The claim is unconditionally true: guidance is prose the
  executing surface ignores, so guidance attached to a step that will run anyway is
  the trap by definition. This avoids the same heuristic class already declined for
  tool names in D-07, and cannot be paraphrased around.

- **D-10:** **Two delivery channels.** `tracing::warn!` on the `mcp.skills` target
  for the infallible path — matching the D-02 exclusion-warning precedent at
  `src/server/skills.rs:1011` — plus structured warnings returned from `build()`, so
  tests assert on data instead of installing a subscriber and callers can act on them.
  No `.strict(true)` escalation this phase (see Deferred).

### Coverage

- **D-11:** **Render everything a manual runner needs; exclude the rest on purpose.**
  Resource-only steps (`WorkflowStep::fetch_resources`), `with_resource` attachments
  and `with_template_binding` bindings all render — a client LLM can read resources
  itself, and a workflow using them would otherwise project to a skill that silently
  omits real steps. `is_retryable()` and `has_task_support()` are server-execution
  mechanics with no manual analogue: excluded deliberately, stated in the module docs,
  and pinned by a test asserting they do NOT appear. This makes SC-3's "every workflow
  fact" a defined universe rather than an open-ended one.

- **D-12:** **Tool name and argument mapping only — no tool descriptions, no input
  schemas.** The client is already connected to this server and has `tools/list`, so
  that information is one call away and always current. Nothing in the digested body
  can drift from the live tool surface, and `ToolInfo` stays a warning-only input
  (D-07).

- **D-13:** **Frontmatter is `name` + `description` only.** Slugified workflow name,
  workflow description verbatim — the minimum agentskills-legal set. Phase 125 serves
  frontmatter verbatim in the `skills/list` entry and hosts verify it field-by-field
  against the fetched file, so a smaller surface is a smaller conformance risk.

### Body contract & naming

- **D-14:** **Golden-pinned per version; the render evolves on minor bumps.** A
  golden-file test pins the current bytes so no change is accidental, and the docs
  state plainly that the exact text is not semver-stable. Every render change is a
  CHANGELOG entry, because a consumer pinning the skill by digest must re-pin —
  spike 010's model makes a digest mismatch a *fatal pre-loop revocation*, not a
  warning. Matches the recorded audience philosophy: core stays additive where cheap,
  the agent tree breaks freely. — **Reversibility:** costly — tightening to a frozen
  contract later is possible; loosening after consumers have pinned is not.

- **D-15:** **Slug fallback warns on `as_skill()`, errors on `build()`.** Normalize
  lossily (`refund_flow` → `refund-flow`). If nothing legal survives, `as_skill()`
  emits a tracing warning and uses a deterministic `workflow-{8 hex of the original
  name}` — always legal, always the same bytes, always traceable back — while
  `build()` returns an error so a strict caller is pushed to rename. **Nothing
  panics:** Phase 125's WR-03 finding was exactly a build-time `panic!` inside a
  `Result`-returning `build()` (`src/server/builder.rs:1501`). Name **collisions need
  no new machinery** — `Skills::into_handler()` already returns
  `Err(Error::Validation)` listing every duplicate `skill://name/SKILL.md` URI
  (`src/server/skills.rs:1176`), and `try_skills` probes for them.

  **D-15a (CORRECTION, 2026-09-02 — the normalization needs a length bound).** Spike
  009's `slugify()` (`.planning/spikes/009-workflow-skill-projection/src/main.rs:68-77`)
  handles case, non-alphanumerics, and leading/trailing/consecutive hyphens correctly
  but has **no length bound**. The authoritative agentskills rule is **1–64
  characters**, `[a-z0-9-]`, no leading or trailing hyphen, no `--`.
  `SequentialWorkflow::new` imposes no length limit on the workflow name, so a
  90-character workflow name would project to a 90-character skill name that a
  conforming host must reject. The normalization must therefore additionally truncate
  to 64 **and then re-strip** any trailing hyphen the truncation created. Truncate only
  *after* the ASCII-reducing map, so `String::truncate` cannot panic on a char
  boundary. This does not change D-15's decision (lossy normalization plus the
  deterministic `workflow-{8 hex}` fallback, warn on `as_skill()` / error on
  `build()`, nothing panics) — it completes the algorithm. The concrete total
  algorithm is in `126-RESEARCH.md` Q3.

  Note the `skills.rs:1176` cite above has drifted: `Skills::into_handler` is now at
  `:1118`, `build_handler` at `:1137`, and the `return Err(Error::validation(msg))` at
  **`:1189`**. The behaviour claim is correct; only the line moved.

- **D-16:** **A new `s45_workflow_skill_projection.rs` example**, `required-features =
  ["skills", "full"]`, alongside the existing s44/c10 pair. It shows a workflow, its
  projected skill, the opt-in prepended prompt and the gate warning in one runnable
  file, and leaves s44's scope as "hand-authored skills".

  **D-16a (CORRECTION, 2026-09-02 — number only, substance unchanged).** The `s45`
  slot is **already taken**: `Cargo.toml:713-717` registers
  `s45_tool_as_task_lifecycle` and `examples/s45_tool_as_task_lifecycle.rs` exists
  (8.0 K). The highest registered example is `s55_handler_logging`
  (`Cargo.toml:831`), and `examples/` also holds s47–s51. **The first free number is
  `s56`**, so the file is `examples/s56_workflow_skill_projection.rs` with a matching
  `[[example]]` stanza placed after `c10_client_skills`. Everything else about D-16 —
  `required-features = ["skills", "full"]`, the four things it must demonstrate, and
  leaving s44 as "hand-authored skills" — is unaffected. The ALWAYS-requirement
  invocation is therefore
  `cargo run --example s56_workflow_skill_projection --features skills,full`, and the
  example must **assert** its invariants, not merely print them.

### Claude's Discretion

The user did not defer any area wholesale. These specific sub-choices were named
during discussion but left to research and planning:

- The `ProjectionWarning` type's exact shape (enum vs struct, whether it carries a
  suggested fix).
- The exact spelling of the D-04 opt-in flag, and whether `task_prompt_handler.rs`
  (60 KB) gets it in the same plan as `prompt_handler.rs` (93 KB).
- Whether a `Skills::from_workflow(&wf)` alias exists alongside D-01's entry point.
- How `DataSource::Constant(Value)` JSON renders, and whether `typed_argument`
  schema information reaches the Inputs section.
- Whether `SequentialWorkflow::validate()` learns the D-09 check (declined as an
  option, but the seam is adjacent).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Positioning and spike evidence (the requirement source)
- `.claude/skills/spike-findings-rust-mcp-sdk/references/skills-positioning-tri-surface.md` —
  the MANIFEST requirement bullets this phase is tracked against (v2.7 has no
  REQUIREMENTS.md yet), the projection mapping, the trust rules, and the spike-011
  decision matrix. Implementation-order item 23 is this phase.
- `.planning/spikes/009-workflow-skill-projection/README.md` — VALIDATED verdict, the
  investigation trail, and the live observation of the post-hoc-judgment trap that
  SC-6 exists to warn about.
- `.planning/spikes/009-workflow-skill-projection/src/main.rs` — the ~90-line
  `project_skill()` at `:101` and `render_data_source()`; the starting point for the
  renderer, not the finished shape (see D-11).
- `.planning/spikes/CONVENTIONS.md` — harness patterns for this territory
  (shared-cell capture for private seams; the in-process `PromptHandler::handle`
  pattern that needs no wire).

### Phase 125 inheritance (the conforming surface this projects INTO)
- `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-CONTEXT.md` —
  D-01..D-11. Load-bearing here: D-02 (the `tracing::warn!` precedent), D-05 (`sha2`
  is at **0.11** — the spike's 0.10-era `format!("{:x}", …)` is not a safe
  copy-paste), D-09 (`make test-skills`), D-10 (capability declaration).
- `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-RESEARCH.md` —
  line-cited architecture map and wire shapes.
- `.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md` —
  the 7-gap blueprint; entry shape and naming rules.
- SEP-2640 current draft: PR `modelcontextprotocol#2640`, head branch
  `sep/skills-extension`, raw markdown — **not** the docs site, which lags.

### Source the phase touches
- `src/server/skills.rs` — `Skill` / `SkillReference` / `Skills` / `SkillEntry`; the
  duplicate-URI rejection at `:1176`; the exclusion warning at `:1011`; the
  `with_reference` / `try_with_reference` precedent at `:362` / `:393`.
- `src/server/workflow/sequential.rs` — the public introspection surface (`name` `:205`,
  `description` `:210`, `arguments` `:215`, `steps` `:220`, `instructions` `:225`,
  `has_task_support` `:200`, `validate` `:239`).
- `src/server/workflow/workflow_step.rs` — `tool` `:299`, `is_resource_only` `:304`,
  `arguments` `:309`, `binding` `:314`, `guidance` `:319`, `resources` `:324`,
  `template_bindings` `:329`, `is_retryable` `:359`.
- `src/server/workflow/data_source.rs` — the three `DataSource` variants at `:11`.
- `src/server/workflow/prompt_handler.rs` (93 KB) and
  `src/server/workflow/task_prompt_handler.rs` (60 KB) — where D-04's opt-in lands.
- `src/types/tools.rs:20` — `ToolAnnotations { read_only_hint, destructive_hint }`,
  the D-07 input.
- `src/server/mod.rs:164` / `:195` — the gating asymmetry: `workflow` is non-wasm but
  ungated, `skills` is `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]`.

### Project standards
- `CLAUDE.md` — the ALWAYS requirements (fuzz, property, unit, `cargo run --example`),
  cognitive complexity ≤ 25, zero SATD, `make quality-gate` before any commit.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **The whole workflow introspection surface is already public** — the projection
  needs zero new SDK accessors to read everything it renders (spike 009 confirmed
  this against the shipped types).
- **`Skills` registry needs no changes** — a projected `Skill` is an ordinary `Skill`;
  it registers, serves and digests exactly like a hand-authored one, and the
  duplicate-name check already exists.
- **`WorkflowPromptHandler::new(workflow, tools, tool_handlers, resource_handler)` is
  public**, so the prompt surface is testable in-process via `PromptHandler::handle` —
  no wire needed for the D-04 prepend tests.
- **`ToolAnnotations` already models exactly what D-07 needs**; nothing new to define.

### Established Patterns
- **Fallible/infallible constructor pairs** (`with_reference` / `try_with_reference`)
  — D-02 follows this rather than inventing a convention.
- **`tracing::warn!` on the `mcp.skills` target** is the established build-time
  diagnostic channel (Phase 125 D-02).
- **Build-time synthesis at the `into_handler()` choke point** — Phase 125 computes
  `Skills::entries()` there, with the limits guard and name-identity validation at the
  same place. A projected skill inherits all of it by construction.
- **Opt-in feature discipline** — `skills` is in neither `default` nor `full`; the
  `full`/`full-v2` enumerated lists and `tests/v1_severability_tripwire.rs` must stay
  untouched.

### Integration Points
- `SequentialWorkflow` gains one `#[cfg(feature = "skills")]` inherent method (D-01) —
  the only skills-shaped intrusion into the ungated workflow module.
- `WorkflowPromptHandler` (and possibly `TaskPromptHandler`) gains the D-04 opt-in.
- `Cargo.toml` gains the `s45` example entry with `required-features = ["skills",
  "full"]`, mirroring the s44/c10 entries at `:704-711`.
- The `make test-skills` leg (Makefile `:1021`) is where the new tests must run —
  `make quality-gate` reaches skills only through it.

</code_context>

<specifics>
## Specific Ideas

- **Determinism landmine — sort the template bindings.**
  `WorkflowStep::template_bindings()` returns `&HashMap<String, DataSource>`, whose
  iteration order is nondeterministic in Rust. D-11 puts template bindings in the
  rendered body, so rendering them unsorted breaks SC-2 byte-equality on its own.
  `arguments()` and the workflow's `arguments()` are `IndexMap`s and are safe by
  insertion order; the `HashMap` is the one exception and must be explicitly ordered.
- **Spike 009's only real bug was Debug-formatting `PromptContent::Text`** when
  rendering the Context section (`main.rs:119` still carries the `{other:?}` fallback).
  Render text properly; decide deliberately what non-`Text` content variants do.
- **`sha2` is at 0.11, not 0.10** — the spike-era `format!("{:x}", …)` digest snippet
  is not a safe copy-paste (Phase 125 D-05 records this as a measured research
  pitfall).
- **Test the registry pass-through on the wire, not just in-process** — SC-4 asks that
  `skill://{name}/SKILL.md` read back byte-identical *through the real handler*
  carrying a conforming SEP-2640 entry, which is Phase 125's contract restated for a
  projected skill.
- **The golden file is the SC-2 determinism test's other half** — re-derivation
  byte-equality proves the renderer is pure; the golden file proves the bytes did not
  change since the last release. D-14 needs both.
- **Surface equivalence (SC-5) is nearly free** — assert that the set of tool names in
  the rendered Procedure equals the set the workflow's steps actually name, and that
  `as_prompt_text() == body` for the projected skill.

</specifics>

<deferred>
## Deferred Ideas

- **Tri-surface decision-matrix docs** (book + course + README) — spike-findings
  implementation-order **item 25**, its own phase. Explicitly declined as an option
  during discussion so 126 does not pull scheduled work forward.
- **`[[skills]]` digest pins on `AgentPackage`** (spike 010) — implementation-order
  **item 24**, its own phase.
- **Making the D-04 prepend default-on** — revisit once the opt-in has real usage.
- **`.strict(true)` on the builder**, escalating D-10 warnings into `build()` errors
  for use as a CI gate — declined this phase to keep the diagnostic surface small.
- **A provenance frontmatter key** (e.g. `x-pmcp-source: workflow:{name}`) so a host
  or pinning agent can tell a skill is a projection — genuinely useful, but how the
  current SEP-2640 draft treats unknown frontmatter keys is unconfirmed and would need
  research before it ships. Blocked on that, not rejected.
- **Freezing the render as a semver-observable contract** (the stricter half of D-14) —
  possible later; not reversible once consumers rely on the looser policy.
- **Tool descriptions / input schemas in the Procedure** (D-12's alternatives) — only
  if a consumer materialises that must read the skill without `tools/list`.

</deferred>

---

*Phase: 126-workflow-to-skill-projection-sequentialworkflow-as-skill*
*Context gathered: 2026-09-02*
