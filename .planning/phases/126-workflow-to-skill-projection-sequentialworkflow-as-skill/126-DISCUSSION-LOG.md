# Phase 126: Workflow to skill projection — `SequentialWorkflow::as_skill()` - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-09-02
**Phase:** 126-workflow-to-skill-projection-sequentialworkflow-as-skill
**Areas discussed:** API shape & gating, Gate warning (SC-6), Coverage scope, Body as contract

---

## API shape & gating

### Q1 — What is the primary entry point for the projection?

| Option | Description | Selected |
|--------|-------------|----------|
| `as_skill()` + builder | `SequentialWorkflow::as_skill()` behind `#[cfg(feature="skills")]` delegating to a `SkillProjection` builder that can carry options later | ✓ |
| `Skills::from_workflow(&wf)` only | Whole renderer inside the gated skills module; no `#[cfg]` on any workflow type; dependency strictly one-directional | |
| `as_skill()` only, no builder | One infallible method, smallest surface; future extra inputs mean a second method or a breaking change | |

**User's choice:** `as_skill()` + builder
**Notes:** Keeps the roadmap's discoverable name while leaving room to accept tool annotations later without a breaking signature change.

### Q2 — Can projection fail? Where does fallibility live?

| Option | Description | Selected |
|--------|-------------|----------|
| Infallible `as_skill()`, fallible `build()` | Mirrors `with_reference` / `try_with_reference` at `src/server/skills.rs:362`/`:393` | ✓ |
| Always fallible `as_skill() -> Result<Skill>` | One entry point, one behaviour; every caller handles an error even for a trivially legal workflow | |
| Never fallible anywhere | Illegal names normalize, collisions get suffixed, problems only surface as tracing warnings | |

**User's choice:** Infallible `as_skill()`, fallible `build()`

### Q3 — Where does the renderer code physically live?

| Option | Description | Selected |
|--------|-------------|----------|
| New `src/server/skills/projection.rs` | Split the 157 KB `skills.rs` into a module dir; workflow keeps a thin delegating `as_skill()` | ✓ |
| Append to existing `src/server/skills.rs` | No restructuring; smallest diff, grows an already-large file | |
| `src/server/workflow/skill_projection.rs` | Reads as "a workflow renders itself"; puts a skills `#[cfg]` inside the ungated workflow module | |

**User's choice:** New `src/server/skills/projection.rs`
**Notes:** The user accepted the placement and then raised a substantive integration question — that a skill is text instructions much like agent instructions, and the workflow binds steps until binding cannot continue automatically. Their proposal: the first message of the workflow prompt should be the skill text, followed by the existing step message list; and the skill should also be readable and executable outside the prompt flow, with the LLM inferring step order from the text. This opened Q4-Q6.

### Q4 — Should the workflow prompt actually lead with the projected skill body in this phase?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, opt-in this phase | Builder flag gated on `feature="skills"` makes message [0] the projected body, replacing the separately-emitted instruction messages; existing transcripts byte-identical unless set | ✓ |
| Yes, default-on this phase | Strongest guarantee; changes `prompts/get` transcripts the moment the feature is enabled, invalidating spike 009's "unchanged execution" property | |
| Not this phase — ship the renderer + seam | Smallest blast radius; leaves the two surfaces sharing a renderer, not a string | |

**User's choice:** Yes, opt-in this phase
**Notes:** Claude flagged three consequences before the choice — self-reference (the prepended body names the prompt it is inside), the byte-equality/digest question if the prepend uses a variant, and duplication of the Context section against the handler's own instruction messages. Claude also flagged the scope consideration: spike 009 recorded "unchanged execution" as a validated property, so changing the transcript should be deliberate.

### Q5 — When should the model surface the prompt to the user?

| Option | Description | Selected |
|--------|-------------|----------|
| At the end, once | The model completes the user's request, then mentions the prompt for next time | ✓ |
| Before starting | Surfaces the cheaper server-side path for this run; interrupts a request already made | |
| Both — offer up front, remind at the end | Maximum discovery, most instruction text, most model discretion | |

**User's choice:** At the end, once
**Notes:** This question exists because of the user's correction on Q4's follow-up. They observed that the skill is read by the LLM and not by the user, and that only the user can trigger a prompt — so a "Server-accelerated alternative" section aimed at the model is not useful. The real reason to carry the pointer is educating users who do not know the prompt workflow exists. Claude confirmed the mechanism: MCP prompts are user-controlled by design, surfaced by the host for a person to select, so the model cannot invoke one. The section therefore became a behavioural instruction rather than a cross-reference.

### Q6 — One byte-identical text everywhere, or canonical + prepend variant?

| Option | Description | Selected |
|--------|-------------|----------|
| One text, conditional closing line | Byte-identical in both positions; the closing line is conditioned on context the model can resolve | ✓ |
| Canonical + suppressed variant | Cleanest reading in both places; two texts, two determinism tests, one digest | |
| One text, unconditional note | Simplest wording; occasionally tells a user who just invoked the prompt that the prompt exists | |

**User's choice:** One text, conditional closing line
**Notes:** An earlier framing of this question offered "drop the section and put the pointer in frontmatter" — Claude flagged that it conflicts with a locked tri-surface MANIFEST requirement. The user's reframing of the section's purpose superseded that framing entirely.

---

## Gate warning (SC-6)

### Q1 — How does the projection decide a step is side-effecting?

| Option | Description | Selected |
|--------|-------------|----------|
| `ToolAnnotations` passed into the builder | `with_tools(...)` reads `read_only_hint` / `destructive_hint` (`src/types/tools.rs:20`); real MCP semantics | ✓ |
| Tool-name heuristic | Verb prefixes; works from a bare `as_skill()`; inherent false positives and a verb list to maintain | |
| New `WorkflowStep::side_effecting(bool)` | Precise and self-documenting; adds public workflow API and only fires for authors who already understood the trap | |

**User's choice:** `ToolAnnotations` passed into the builder
**Notes:** Consequence accepted — a bare `as_skill()` holds no tool map and cannot warn.

### Q2 — Both hints are `Option<bool>` and most servers never set them. What counts as side-effecting?

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit signal + one "unverifiable" note | Warn on an explicit hint; separate one-line diagnostic when a guidance-bearing step's tool has no annotations at all | ✓ |
| Explicit signal only | Zero noise; a server that never annotates gets no signal, including the refund_flow shape spike 009 caught live | |
| MCP spec defaults | Absent `read_only_hint` means not read-only, so every unannotated tool counts — fires on essentially every existing workflow | |

**User's choice:** Explicit signal + one "unverifiable" note

### Q3 — What counts as "gate-like" guidance on a side-effecting step?

| Option | Description | Selected |
|--------|-------------|----------|
| Any guidance on a side-effecting step | No text analysis; the claim is unconditionally true and cannot be paraphrased around | ✓ |
| Gate-language keyword match | Lower volume, closer to SC-6's literal wording; needs a maintained phrase list | |
| Always warn, escalate on gate language | Best signal; reintroduces the phrase list | |

**User's choice:** Any guidance on a side-effecting step
**Notes:** Consistent with Q1 — the same heuristic class was declined for tool names.

### Q4 — `as_skill()` is infallible, so what does "warning" mean?

| Option | Description | Selected |
|--------|-------------|----------|
| `tracing::warn!` + structured warnings from `build()` | Matches the Phase 125 D-02 precedent at `src/server/skills.rs:1011`; tests assert on data, not a subscriber | ✓ |
| `tracing::warn!` only | Exact Phase 125 parity; nothing for a caller to inspect | |
| Structured warnings + `.strict(true)`, no tracing | Usable as a CI gate; a plain `as_skill()` author never learns about the trap | |
| All three | Covers every consumer; most surface to design, document and test for a diagnostic | |

**User's choice:** `tracing::warn!` + structured warnings from `build()`

---

## Coverage scope

### Q1 — What does "every workflow fact" cover?

| Option | Description | Selected |
|--------|-------------|----------|
| Everything a manual runner needs; rest excluded on purpose | Resource-only steps, `with_resource`, template bindings render; `retryable` / `has_task_support` excluded and pinned by a negative test | ✓ |
| Render everything, exhaustively | Most literal reading of SC-3; puts server-execution mechanics into a manual-procedure document | |
| Tool steps only, as spiked | Smallest renderer; a workflow using `fetch_resources` projects to a skill that omits real steps | |

**User's choice:** Everything a manual runner needs; rest excluded on purpose

### Q2 — How much `ToolInfo` reaches the Procedure?

| Option | Description | Selected |
|--------|-------------|----------|
| Tool name + argument mapping only | Client has `tools/list`; nothing in the digested body can drift from the live tool surface | ✓ |
| Add the tool's description line | Helps a cold read; the digest then pins a description that may change independently | |
| Description + input schema summary | Fully self-contained; largest body and largest drift surface | |

**User's choice:** Tool name + argument mapping only

### Q3 — What lands in the projected skill's YAML frontmatter?

| Option | Description | Selected |
|--------|-------------|----------|
| `name` + `description` only | Minimum agentskills-legal set; smallest conformance surface given hosts verify frontmatter field-by-field | ✓ |
| `name` + `description` + provenance key | Lets a host or pinning agent tell a skill is a projection; unknown-key handling in the current SEP draft unconfirmed | |
| Fuller set — licence, version, provenance | Most informative; a version bump would change every projected skill's digest | |

**User's choice:** `name` + `description` only

---

## Body as contract

### Q1 — Is the rendered markdown a stable contract across releases?

| Option | Description | Selected |
|--------|-------------|----------|
| Golden-pinned per version, evolves on minor bumps | No accidental change; docs state the bytes are not semver-stable; every render change is a CHANGELOG entry | ✓ |
| Frozen — bytes are semver-observable | Safest for spike 010's fatal-revocation pin model; a typo fix in generated prose becomes a 3.0 | |
| Internal — structural assertions only | Maximum renderer freedom; a digest-pinning agent hard-fails on a patch release with no warning | |

**User's choice:** Golden-pinned per version, evolves on minor bumps

### Q2 — What happens to a workflow name with no legal slug?

| Option | Description | Selected |
|--------|-------------|----------|
| Fallback + warn on `as_skill()`, error on `build()` | Deterministic `workflow-{8 hex}` fallback; nothing panics (Phase 125 WR-03 was exactly that defect) | ✓ |
| Normalize only; let the registry reject it | One failure path; the error names a URI, not the workflow that caused it | |
| Catch it in `SequentialWorkflow::validate()` | Error closest to the mistake; imposes skill-naming rules on authors who never enable `skills` | |

**User's choice:** Fallback + warn on `as_skill()`, error on `build()`
**Notes:** Claude established before asking that name *collisions* need no new machinery — `Skills::into_handler()` already returns `Err(Error::Validation)` listing every duplicate `skill://name/SKILL.md` URI (`src/server/skills.rs:1176`).

### Q3 — Where does the ALWAYS-requirement example live?

| Option | Description | Selected |
|--------|-------------|----------|
| New s-prefixed example | `s45_workflow_skill_projection.rs`, `required-features = ["skills", "full"]`; keeps s44 as "hand-authored skills" | ✓ |
| Extend the existing `s44_server_skills` | No new Cargo.toml entry; one example carrying two teaching jobs | |
| New example plus a book/course section | Matches the three-shapes docs rule; pulls implementation-order item 25 forward into 126 | |

**User's choice:** New s-prefixed example

---

## Claude's Discretion

The user did not defer any area wholesale. These sub-choices were named during
discussion and left to research and planning:

- The `ProjectionWarning` type's shape (enum vs struct; whether it carries a suggested fix).
- The exact spelling of the prompt-prepend opt-in flag, and whether
  `task_prompt_handler.rs` gets it in the same plan as `prompt_handler.rs`.
- Whether a `Skills::from_workflow(&wf)` alias exists alongside `as_skill()`.
- How `DataSource::Constant(Value)` JSON renders, and whether `typed_argument` schema
  information reaches the Inputs section.
- Whether `SequentialWorkflow::validate()` learns the side-effect check.

## Deferred Ideas

- Tri-surface decision-matrix docs (book + course + README) — implementation-order item 25, its own phase.
- `[[skills]]` digest pins on `AgentPackage` (spike 010) — implementation-order item 24, its own phase.
- Making the prompt prepend default-on.
- `.strict(true)` on the builder, escalating warnings into `build()` errors as a CI gate.
- A provenance frontmatter key (`x-pmcp-source: workflow:{name}`) — blocked on confirming how the current SEP-2640 draft treats unknown frontmatter keys, not rejected.
- Freezing the render as a semver-observable contract.
- Tool descriptions / input schemas in the Procedure.
