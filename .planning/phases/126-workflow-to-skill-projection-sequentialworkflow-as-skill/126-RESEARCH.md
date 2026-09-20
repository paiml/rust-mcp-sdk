# Phase 126: Workflow to skill projection — `SequentialWorkflow::as_skill()` - Research

**Researched:** 2026-09-02
**Domain:** Rust SDK internals — deterministic text rendering, SEP-2640 skill conformance, feature-gated module split
**Confidence:** HIGH (every claim below is a `Read`/`grep`/executed-command observation against the live tree at HEAD `8bfc318a`)

---

## Summary

Everything the renderer needs to read is already public and already deterministic, with **exactly one exception** — `WorkflowStep::template_bindings()` returns a `&HashMap`, and D-11 puts it in the body. Everything the projected skill needs downstream (verbatim frontmatter, `{uri, digest, size}` manifest, name-identity validation, duplicate-URI rejection) is inherited from Phase 125 by construction: a projected `Skill` is an ordinary `Skill`. The module split (D-03) is genuinely internal — no `mod`/`pub use` line moves, because `src/server/mod.rs:194-195` already says `pub mod skills;` and `src/lib.rs` contains **zero** references to skills.

Three locked decisions are contradicted by the live tree and are recorded in `## Conflicts With Locked Decisions` rather than planned around: **D-04's "replacing the separately-emitted `instructions()` messages" describes something that does not exist** (`SequentialWorkflow::instructions()` reaches no production code path at all); **D-16's `s45` slot is already occupied** by `s45_tool_as_task_lifecycle`; and the spike's `slugify()` — the starting point D-15 normalizes from — does not enforce the agentskills `1-64 character` bound, so it can emit an illegal name.

The riskiest planning surface is not the renderer, it is the **test-reachability geometry**. `make test-skills` uses `SKILLS_FEATURES := skills,streamable-http,http-client,testing` (`Makefile:954`) with four separately-guarded selectors and a zero-test-count guard; `make test-property` and `make test-unit` pin `--features "full"`, which **excludes** `skills`, so a skills property test placed the "normal" way runs zero tests and reports green. And `cargo fuzz run` **cannot execute on this machine's stable toolchain** — measured, output pasted below — while `make test-fuzz` swallows that failure and exits 0.

**Primary recommendation:** split `skills.rs` → `skills/mod.rs` + `skills/projection.rs` via `git mv` (mod.rs style, 22:1 in-repo precedent), build the renderer as ~7 small `fn render_*` helpers to stay under PMAT cog 25 (the gate is at **0 violations** today — measured), put every new unit/proptest inside `server::skills::projection::tests` (so the `--lib skills` selector reaches it), put the SC-4 wire proof in `tests/skills_routing.rs` and the byte-identity proof in `tests/skills_integration.rs` (so selectors 3 and 4 reach them), and take D-04 back to the user before planning it.

---

## Project Constraints (from CLAUDE.md)

| Directive | Source | Effect on this phase |
|---|---|---|
| ZERO tolerance for defects; `make quality-gate` before any commit | CLAUDE.md § Toyota Way | Every plan's final verify must be `make quality-gate`, not a narrower `cargo test` |
| Cognitive complexity ≤ 25 per function; CI runs `pmat quality-gate --fail-on-violation --checks complexity` (PMAT pinned 3.15.0) | CLAUDE.md § CI Quality Gates | **MEASURED: gate currently PASSES with `Total violations: 0`.** Any new cog>25 function is a *new* CI failure. A monolithic `project_skill()` is the exact shape that trips it. |
| Zero SATD comments | CLAUDE.md; `Makefile:1996-1999` `check-todos` greps `src/` for `TODO\|FIXME\|HACK\|XXX` | No placeholder comments in `projection.rs` |
| ALWAYS: FUZZ + PROPERTY + UNIT + `cargo run --example` | CLAUDE.md § ALWAYS Requirements | Four artifacts required; see `## Validation Architecture` for where each must live to be reachable |
| Doctests on all public APIs | CLAUDE.md; `Makefile:1504-1508` `doc-check` runs `RUSTDOCFLAGS="-D warnings" cargo doc --features ...,skills,...` | `#![warn(missing_docs)]` at `src/lib.rs:24-29`. Every new `pub` item needs rustdoc; every doctest must self-gate with `# #[cfg(feature = "skills")] { … # }` (precedent: `src/server/skills.rs:528`, `:970`, `src/server/mod.rs:4670`) |
| Tests run with `--test-threads=1` | CLAUDE.md § Development Workflow | Every verify command in every plan |
| Contract-first (`../provable-contracts/contracts/<crate>/`, `pmat comply check`) | CLAUDE.md § Contract-First | `make comply` is chained into `quality-gate` (`Makefile:1921`). Wave 0 should check whether a `pmcp` skills contract exists. |

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|---|---|---|---|
| Workflow → markdown rendering | SDK library (`src/server/skills/projection.rs`) | — | Pure function over already-public introspection; D-03 places it with the `skills` feature so `make test-skills`/`lint-skills` reach it |
| `as_skill()` entry point | SDK library (`src/server/workflow/sequential.rs`) | delegates to projection | D-01; thin `#[cfg(feature = "skills")]` inherent method |
| Slug normalization | SDK library (projection) | — | D-15 / SC-1; must be owned here because the workflow name is not agentskills-legal |
| Name-identity enforcement (`name` == final URI segment) | SDK library (`skills.rs:1444` `validate_names`) | — | **Already exists.** SC-1 holds by construction if the projection uses `Skill::new(slug, body)` with no `.with_path()` |
| Digest / size / verbatim frontmatter manifest | SDK library (`skills.rs:1487` `skill_resource_manifest`) | — | Inherited from Phase 125 untouched |
| Gate warning (SC-6) | SDK library (`SkillProjection` builder) | caller supplies `ToolAnnotations` | D-07: the annotations live on `crate::types::ToolInfo`, which the projection does **not** otherwise hold |
| Prompt-transcript prepend | SDK library (`prompt_handler.rs`, `task_prompt_handler.rs`) | — | D-04; **two separate code paths**, see Q4 |
| Byte-level render contract | Repo test fixture (golden file) + CHANGELOG | — | D-14 |

---

## User Constraints (from CONTEXT.md)

### Locked Decisions

> Copied verbatim from `126-CONTEXT.md` `<decisions>`. Research fills in HOW; it never re-opens WHAT.

**API shape & gating**

- **D-01:** **`SequentialWorkflow::as_skill()` plus a `SkillProjection` builder.** `as_skill()` lives behind `#[cfg(feature = "skills")]` on the ungated `SequentialWorkflow` and delegates to the builder; the builder is what carries options (tool annotations today, section overrides later). Keeps the roadmap's discoverable name and leaves room to grow inputs without a breaking signature change. — **Reversibility:** costly — adding builder options later is additive, but renaming or removing a public inherent method on a published 2.x type is a major-version event.
- **D-02:** **Fallibility splits along the two entry points.** `as_skill() -> Skill` never fails; the builder's `build() -> Result<Skill>` is where strict checks live. This mirrors the module's own precedent — `Skill::with_reference` / `try_with_reference` at `src/server/skills.rs:362` and `:393`. — **Reversibility:** one-way — moving `as_skill()` to `Result<Skill>` later breaks every call site on the published 2.x line.
- **D-03:** **Renderer lives in a new `src/server/skills/projection.rs`.** Split `skills.rs` (already 157 KB) into a module directory; the workflow module keeps only the thin delegating `as_skill()`. The renderer ships with the `skills` feature, so `make test-skills` covers it, and PMAT cognitive complexity stays contained in a fresh file. The canonical public path remains `pmcp::server::skills::*`. — **Reversibility:** reversible — internal file layout; no public path moves.
- **D-04:** **The prompt prepend lands this phase, opt-in.** A `WorkflowPromptHandler` opt-in (builder flag, gated on `feature = "skills"`) makes message [0] the projected body, **replacing** the separately-emitted `instructions()` messages so the workflow's system prompt does not appear twice in one transcript. Without the flag every existing transcript is byte-identical, so spike 009's "unchanged execution" property still holds for current users. Rationale: shipping the renderer without its consumer leaves the anti-drift claim theoretical — with the flag the two surfaces share a *string*, not merely a renderer. — **Reversibility:** costly — once the flag exists, callers depend on the transcript shape it produces.

**The dual-surface text**

- **D-05:** **One byte-identical body everywhere.** The same bytes are served at `skill://{name}/SKILL.md` and prepended as message [0]. One string, one digest, no variant to keep in sync — no suppressed-section render. — **Reversibility:** costly — the digest published in `skills/list` binds pinning consumers to these bytes (see D-14).
- **D-06:** **The closing pointer is a user-education instruction to the model, not a cross-reference for a reader.** MCP prompts are user-controlled by design: the host surfaces them for a person to select, and the model cannot invoke one. A section telling the model to "prefer the prompt" is therefore inert text. Its only useful job is closing the *discovery* gap for users who do not know the prompt workflow exists. So the body carries one line instructing the model to mention, **once, at the end**, that this server also offers the `{workflow}` prompt which runs these steps server-side. The line is conditioned on context the model can actually resolve — a prompt result carries executed step results, a cold skill read does not — so the same bytes read correctly in both positions. It names the prompt and never redirects, satisfying the locked tri-surface requirement more literally than the spike's "Server-accelerated alternative" wording did. — **Reversibility:** reversible as prose, but any edit changes the digest (D-14).

**Gate warning (SC-6)**

- **D-07:** **Side-effect detection reads `ToolAnnotations` supplied to the builder.** `SkillProjection::new(&wf).with_tools(...)` consults the shipped `ToolAnnotations { read_only_hint, destructive_hint }` (`src/types/tools.rs:20`) rather than inventing semantics. Consequence to document: a bare `as_skill()` holds no tool map and therefore cannot warn — the warning is a builder-path capability.
- **D-08:** **Explicit signal warns; missing annotations get a distinct "unverifiable" note.** Warn when `read_only_hint == Some(false)` or `destructive_hint == Some(true)`. When a guidance-bearing step's tool carries no annotations at all, emit a *separate* one-line diagnostic saying the check could not be performed. Rejected: following MCP's literal defaults (absent `read_only_hint` means not read-only), which would fire on essentially every existing workflow and is how a real warning gets muted.
- **D-09:** **Any guidance on a side-effecting step trips the warning — no text analysis, no phrase list.** The claim is unconditionally true: guidance is prose the executing surface ignores, so guidance attached to a step that will run anyway is the trap by definition. This avoids the same heuristic class already declined for tool names in D-07, and cannot be paraphrased around.
- **D-10:** **Two delivery channels.** `tracing::warn!` on the `mcp.skills` target for the infallible path — matching the D-02 exclusion-warning precedent at `src/server/skills.rs:1011` — plus structured warnings returned from `build()`, so tests assert on data instead of installing a subscriber and callers can act on them. No `.strict(true)` escalation this phase (see Deferred).

**Coverage**

- **D-11:** **Render everything a manual runner needs; exclude the rest on purpose.** Resource-only steps (`WorkflowStep::fetch_resources`), `with_resource` attachments and `with_template_binding` bindings all render — a client LLM can read resources itself, and a workflow using them would otherwise project to a skill that silently omits real steps. `is_retryable()` and `has_task_support()` are server-execution mechanics with no manual analogue: excluded deliberately, stated in the module docs, and pinned by a test asserting they do NOT appear. This makes SC-3's "every workflow fact" a defined universe rather than an open-ended one.
- **D-12:** **Tool name and argument mapping only — no tool descriptions, no input schemas.** The client is already connected to this server and has `tools/list`, so that information is one call away and always current. Nothing in the digested body can drift from the live tool surface, and `ToolInfo` stays a warning-only input (D-07).
- **D-13:** **Frontmatter is `name` + `description` only.** Slugified workflow name, workflow description verbatim — the minimum agentskills-legal set. Phase 125 serves frontmatter verbatim in the `skills/list` entry and hosts verify it field-by-field against the fetched file, so a smaller surface is a smaller conformance risk.

**Body contract & naming**

- **D-14:** **Golden-pinned per version; the render evolves on minor bumps.** A golden-file test pins the current bytes so no change is accidental, and the docs state plainly that the exact text is not semver-stable. Every render change is a CHANGELOG entry, because a consumer pinning the skill by digest must re-pin — spike 010's model makes a digest mismatch a *fatal pre-loop revocation*, not a warning. Matches the recorded audience philosophy: core stays additive where cheap, the agent tree breaks freely. — **Reversibility:** costly — tightening to a frozen contract later is possible; loosening after consumers have pinned is not.
- **D-15:** **Slug fallback warns on `as_skill()`, errors on `build()`.** Normalize lossily (`refund_flow` → `refund-flow`). If nothing legal survives, `as_skill()` emits a tracing warning and uses a deterministic `workflow-{8 hex of the original name}` — always legal, always the same bytes, always traceable back — while `build()` returns an error so a strict caller is pushed to rename. **Nothing panics:** Phase 125's WR-03 finding was exactly a build-time `panic!` inside a `Result`-returning `build()` (`src/server/builder.rs:1501`). Name **collisions need no new machinery** — `Skills::into_handler()` already returns `Err(Error::Validation)` listing every duplicate `skill://name/SKILL.md` URI (`src/server/skills.rs:1176`), and `try_skills` probes for them.
- **D-16:** **A new `s45_workflow_skill_projection.rs` example**, `required-features = ["skills", "full"]`, alongside the existing s44/c10 pair. It shows a workflow, its projected skill, the opt-in prepended prompt and the gate warning in one runnable file, and leaves s44's scope as "hand-authored skills".

### Claude's Discretion

The user did not defer any area wholesale. These specific sub-choices were named during discussion but left to research and planning:

- The `ProjectionWarning` type's exact shape (enum vs struct, whether it carries a suggested fix).
- The exact spelling of the D-04 opt-in flag, and whether `task_prompt_handler.rs` (60 KB) gets it in the same plan as `prompt_handler.rs` (93 KB).
- Whether a `Skills::from_workflow(&wf)` alias exists alongside D-01's entry point.
- How `DataSource::Constant(Value)` JSON renders, and whether `typed_argument` schema information reaches the Inputs section.
- Whether `SequentialWorkflow::validate()` learns the D-09 check (declined as an option, but the seam is adjacent).

### Deferred Ideas (OUT OF SCOPE)

- **Tri-surface decision-matrix docs** (book + course + README) — spike-findings implementation-order **item 25**, its own phase. Explicitly declined as an option during discussion so 126 does not pull scheduled work forward.
- **`[[skills]]` digest pins on `AgentPackage`** (spike 010) — implementation-order **item 24**, its own phase.
- **Making the D-04 prepend default-on** — revisit once the opt-in has real usage.
- **`.strict(true)` on the builder**, escalating D-10 warnings into `build()` errors for use as a CI gate — declined this phase to keep the diagnostic surface small.
- **A provenance frontmatter key** (e.g. `x-pmcp-source: workflow:{name}`) so a host or pinning agent can tell a skill is a projection — genuinely useful, but how the current SEP-2640 draft treats unknown frontmatter keys is unconfirmed and would need research before it ships. Blocked on that, not rejected.
- **Freezing the render as a semver-observable contract** (the stricter half of D-14) — possible later; not reversible once consumers rely on the looser policy.
- **Tool descriptions / input schemas in the Procedure** (D-12's alternatives) — only if a consumer materialises that must read the skill without `tools/list`.

> **Research note on the deferred provenance key — the blocker is now RESOLVED (informational only; still out of scope).** The SEP-2640 draft delegates the frontmatter schema entirely to the Agent Skills spec, which defines an **optional `metadata` field: "A map from string keys to string values… Clients can use this to store additional properties not defined by the Agent Skills spec. We recommend making your key names reasonably unique to avoid accidental conflicts"** `[CITED: agentskills.io/specification#metadata-field]`. SEP-2640 adds: *"Within the frontmatter `metadata` object, keys prefixed with `io.modelcontextprotocol/` are reserved for metadata defined by MCP extensions. This extension currently defines no such keys. Implementations SHOULD ignore keys under this prefix that they do not recognize."* `[CITED: seps/2640-skills-extension.md:243, branch sep/skills-extension, fetched 2026-09-02; PR head last pushed 2026-08-29T18:46:46Z]`. So the legal home is `metadata: { <vendor-key>: <string> }`, **not** a bare top-level frontmatter key. D-13 locks frontmatter to `name` + `description` this phase; recording it here so the deferred item is unblocked when it is scheduled.

---

## Phase Requirements

v2.7 has no `REQUIREMENTS.md` (`.planning/STATE.md` states it explicitly). The tracked requirement set is the MANIFEST bullets in `.claude/skills/spike-findings-rust-mcp-sdk/references/skills-positioning-tri-surface.md` § Requirements.

| MANIFEST bullet | Satisfied by Phase 126? | Research support |
|---|---|---|
| **"Workflow↔skill is a projection, not a rival feature."** Ship `SequentialWorkflow::as_skill()` (SDK-owned renderer, deterministic, byte-equal on re-derivation) so the two surfaces cannot drift. Generated body stays a COMPLETE manual procedure; the closing section cross-references the workflow prompt but never redirects. | **YES — fully** | D-01/D-05/D-06 + SC-2/SC-3. Determinism analysis in Q2; the one nondeterministic accessor (`template_bindings`) named with its sort key |
| **"Projection owns name slugification."** Workflow names are not agentskills-legal (`refund_flow` → `refund-flow`); the final URI segment must equal the skill name. | **YES — fully** | Q3: the authoritative agentskills rule set is now quoted; `Skill::new(slug, body)` makes the URI segment identity hold by construction (`skills.rs:421-427`) |
| **"Post-hoc judgment is the workflow surface's blind spot."** Projection-time warning required when a side-effecting step carries gate-like guidance. | **YES — fully** | D-07..D-10 + SC-6. Q6 confirms the annotation surface and names the realistic input type |
| **"AgentPackage digest pins ARE SEP-2640 content-bound approval."** | **NO — deferred, item 24** | Out of scope per `<deferred>` |
| **"System-prompt placement is a privilege decision."** | **NO — pmcp-agent side, item 24** | Out of scope |
| **"Positioning is settled: alongside, composing — never instead."** | **PARTIAL** — the *code* expression lands (D-06 closing line, SC-5 surface equivalence); the *docs* expression (decision matrix in book/course/README) is item 25, deferred | `<deferred>` |

---

## Conflicts With Locked Decisions

> Three findings contradict a locked decision or a stated fact in CONTEXT.md. Per the research constraints these are recorded, not planned around. **The planner must resolve #1 with the user before writing the D-04 plan.**

### CONFLICT 1 (BLOCKING) — D-04's "separately-emitted `instructions()` messages" do not exist

**D-04 says:** message [0] becomes the projected body, *"**replacing** the separately-emitted `instructions()` messages so the workflow's system prompt does not appear twice in one transcript."*

**Measured:** `SequentialWorkflow::instructions()` (`src/server/workflow/sequential.rs:225-227`) reaches **no production code path in the crate**.

```
$ grep -rn "\.instructions()" src/ examples/ tests/
src/server/workflow/sequential.rs:321:        assert_eq!(workflow.instructions().len(), 2);
src/server/workflow/sequential.rs:417:        assert_eq!(workflow.instructions().len(), 1);
examples/s31_workflow_minimal.rs:127:            for (i, instruction) in workflow.instructions().iter().enumerate() {
```

The two `sequential.rs` hits are inside `#[cfg(test)] mod tests`. `grep -n "instructions" src/server/workflow/prompt_handler.rs src/server/workflow/task_prompt_handler.rs` returns **`0 matches`**. `InternalPromptMessage::to_protocol` (`src/server/workflow/conversion.rs:116`) — the only converter that could put an instruction on the wire — has callers only in its own `#[cfg(test)] mod tests` (`conversion.rs:168,183,203,222,241`) and one rustdoc example (`src/server/mod.rs:855`).

The actual `GetPromptResult` message sequence built by `WorkflowPromptHandler::handle` (`prompt_handler.rs:750-957`) is:

| index | source | line |
|---|---|---|
| [0] | `create_user_intent` — `PromptMessage::user("I want to {description}.\nParameters:\n  - k: \"v\"")` | pushed `:771`, built `:406-427` |
| [1] | `create_assistant_plan` — `PromptMessage::assistant("Here's my plan:\n1. {tool} - {tool_info.description}\n…")` | pushed `:774`, built `:429-461` |
| [2..] | per step: optional guidance (`:805`), resource messages (`:816`/`:846`), tool-call announcement (`:887`), tool result (`:893`) | — |

**Consequence for the plan:** there is nothing to remove, so the flag reduces to a pure *prepend*. **But the duplication hazard D-04 was written to avoid is real and lands somewhere else:** the projected body's `## Procedure` section enumerates the same steps and the same tool names that `create_assistant_plan` emits at [1], and the projected frontmatter/heading carries the same `description` that `create_user_intent` emits at [0]. A prepend with no suppression therefore *does* produce the "appears twice" transcript D-04 objects to — just against messages [0] and [1] rather than against instruction messages.

**What the planner must get decided (do not choose silently):** with the flag on, does the handler (a) prepend only, accepting the [0]/[1] overlap; (b) prepend and suppress `create_assistant_plan`; or (c) prepend and suppress both `create_user_intent` and `create_assistant_plan`? Option (c) removes the `Parameters:` block, which is the only place the caller's actual argument *values* appear — the projected body contains argument *specs*, not values — so (c) loses information the transcript currently carries. Recommend (b) as the reading closest to D-04's stated intent, but it is a user decision.

**Second-order note (out of scope, worth recording):** `SequentialWorkflow::instruction()` (`sequential.rs:168`) is a shipped public builder method whose value is silently dropped by every served surface. `examples/s31_workflow_minimal.rs:127` prints instructions and thereby implies they are live. That is a latent SDK defect independent of Phase 126.

### CONFLICT 2 — D-16's `s45` slot is already taken

**D-16 names** `s45_workflow_skill_projection.rs`. `Cargo.toml:713-717` reads verbatim:

```toml
[[example]]
name = "s45_tool_as_task_lifecycle"
path = "examples/s45_tool_as_task_lifecycle.rs"
required-features = ["full"]
```

`examples/s45_tool_as_task_lifecycle.rs` exists (8.0 K). The highest registered example is `s55_handler_logging` (`Cargo.toml:831`); `examples/` also holds `s47_v2_stateless_mrtr`, `s48_durable_poll_decision`, `s48_v2_mrtr_client`, `s49_v2_subscriptions_client`, `s50_v2_tasks_server`, `s51_v2_tasks_agent`. **The first free number is `s56`.** D-16's *substance* (a new example, `required-features = ["skills", "full"]`, mirroring the s44/c10 pair, leaving s44 as "hand-authored skills") is unaffected — only the number must change. Recommend `s56_workflow_skill_projection`.

### CONFLICT 3 — the spike's `slugify()` can emit an agentskills-ILLEGAL name

D-15 says "Normalize lossily (`refund_flow` → `refund-flow`)", pointing at spike 009's `slugify` (`.planning/spikes/009-workflow-skill-projection/src/main.rs:68-77`). That function handles case, non-alnum, and leading/trailing/consecutive hyphens correctly — but has **no length bound**. The authoritative rule (quoted in Q3) is `1-64 characters`. `SequentialWorkflow::new` imposes no length limit on the name, so a 90-character workflow name projects to a 90-character skill name that a conforming host must reject.

This does not contradict D-15's *decision* (lossy normalization + `workflow-{8 hex}` fallback); it means the normalization must additionally truncate to 64 and re-strip a trailing hyphen created by the truncation. The concrete algorithm is in Q3.

### Line-cite drift found in CONTEXT.md (informational — all cites still resolve to the right thing)

| CONTEXT cite | Actual | Note |
|---|---|---|
| `skills.rs:1176` "duplicate-URI rejection" | `Skills::into_handler` is at `:1118`; `build_handler` (which holds the check) at `:1137`; the `return Err(Error::validation(msg))` at **`:1189`** | `:1176` is inside `build_handler`'s cross-collision comment block. Behaviour claim is correct. |
| `Cargo.toml:704-711` "s44/c10 entries" | s44 stanza `:703-706`, c10 stanza `:708-711` | Cite is one line short at the head |
| `src/server/mod.rs:164 / :195` "gating asymmetry" | `#[cfg(not(target_arch = "wasm32"))]` `:164` + `pub mod workflow;` `:165`; `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` `:194` + `pub mod skills;` `:195` | Correct |
| All `sequential.rs` / `workflow_step.rs` / `data_source.rs` cites | **Every one resolves exactly** (`:200,205,210,215,220,225,239` and `:294,299,304,309,314,319,324,329,359` and `:11`) | No drift |
| `skills.rs:362` / `:393` / `:1011` | All exact | No drift |
| `src/types/tools.rs:20` | `pub struct ToolAnnotations {` is at `:20` | Exact |
| `src/server/builder.rs:1501` (WR-03 panic) | `panic!("Skills: {e}; use try_skills(...) …")` at `:1501` inside `finalize_skills_resources` (`:1487`) | Exact — still open |
| `Makefile:1021` (`make test-skills`) | `.PHONY: test-skills` `:1021`, `test-skills:` `:1022` | Exact |

---

## Research Questions — Answers

### Q1. Module split (D-03)

**Layout: use `mod.rs`.** In-repo convention is overwhelming — 22 directories use `<dir>/mod.rs`, exactly **one** uses the 2018 `X.rs + X/` form:

```
MOD.RS:     src/server/mod.rs, src/server/auth/mod.rs, src/server/workflow/mod.rs,
            src/server/observability/mod.rs, src/server/mcp_apps/mod.rs,
            src/types/mod.rs, src/types/protocol/mod.rs, src/client/mod.rs, … (22 total)
2018-STYLE: src/server/streamable_http_server.rs + src/server/streamable_http_server/   (1 total)
```

`[VERIFIED: enumerated by script over src/ this session]`

**The mechanically safe sequence:**

1. `git mv src/server/skills.rs src/server/skills/mod.rs` (preserves blame; a copy+delete does not).
2. `touch src/server/skills/projection.rs`; add to `skills/mod.rs`:
   ```rust
   /// Deterministic `SequentialWorkflow` → SEP-2640 `Skill` projection.
   pub mod projection;
   pub use projection::{SkillProjection, ProjectionWarning};   // names TBD by the planner
   ```
3. **Nothing else moves.** `src/server/mod.rs:194-195` already reads
   ```rust
   #[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
   pub mod skills;
   ```
   and Rust resolves `skills` to either `skills.rs` or `skills/mod.rs` identically. `src/server/mod.rs:200-201` re-exports `pub use skills::{Skill, SkillReference, Skills};` — unchanged. **`grep -n "skills" src/lib.rs` returns `0 matches`** — the crate root does not mention skills at all, so no `lib.rs` edit exists to get wrong.

**What breaks if done wrong:**

| Mistake | Symptom |
|---|---|
| Creating both `skills.rs` *and* `skills/mod.rs` | `error[E0761]: file for module 'skills' found at both …` |
| Moving `pub(crate)` items into `projection.rs` without re-exporting | `SkillsHandler`, `SkillDiagnostic`, `entries_with_diagnostics`, `finalize`, `Skill::resolved_path`/`skill_md_uri`/`reference_uri` are consumed from `src/server/core.rs`, `src/server/builder.rs`, `src/server/mod.rs`, `src/server/streamable_http_server.rs`. Keep every one of them in `skills/mod.rs`. |
| Changing the public path | `pmcp::server::skills::*` and `pmcp::server::{Skill, SkillReference, Skills}` are used by `examples/s44_server_skills.rs`, `examples/c10_client_skills.rs`, `tests/skills_integration.rs:38`, `tests/skills_routing.rs`, `fuzz/fuzz_targets/fuzz_skill_entry.rs`, and the pmcp-book/pmcp-course chapters |
| Assuming `projection.rs` cannot see `Skill`'s private fields | It can — Rust privacy is "visible in the defining module **and its descendants**". `projection.rs` is a child of `server::skills`, so `super::sha256_digest_hex` (`skills/mod.rs:1801`, crate-private) and `Skill`'s private fields are both reachable. Reuse `sha256_digest_hex` for D-15's `workflow-{8 hex}` fallback rather than re-implementing it. |

**No functional dependency on the literal path exists.** The only mentions of the string `src/server/skills.rs` outside `.planning/` are comments: `Cargo.toml:154`, `Cargo.toml:311`, `Makefile:906`, `:956`, `:1501`, `:1913`. Update them for accuracy; nothing fails if they lag.

**Test-selector consequence (load-bearing).** `make test-skills` selector 1 is `cargo test -p pmcp --features "$(SKILLS_FEATURES)" --lib skills` (`Makefile:1024`) — a substring filter on the full test path. Tests placed at `server::skills::projection::tests::*` match "skills" ✓. Tests placed at `server::workflow::sequential::tests::as_skill_*` do **not** contain the substring "skills" ("skill" ≠ "skills") and would be silently excluded. **Put every projection unit test inside `src/server/skills/projection.rs`'s `mod tests`.**

### Q2. The render (D-05, D-11, D-12, D-13)

**Live accessor inventory** — every one is public, non-`#[cfg]`-gated, and available today. `src/server/workflow/` contains **zero** `#[cfg(feature = …)]` items (`grep` returned no matches), so nothing here is feature-conditional; the whole module is `#[cfg(not(target_arch = "wasm32"))]` at `src/server/mod.rs:164`.

| Accessor | Return type | Line | Determinism |
|---|---|---|---|
| `SequentialWorkflow::name` | `&str` | `sequential.rs:205` | ✓ |
| `SequentialWorkflow::description` | `&str` | `:210` | ✓ |
| `SequentialWorkflow::arguments` | `&IndexMap<ArgName, ArgumentSpec>` | `:215` | ✓ insertion order |
| `SequentialWorkflow::steps` | `&[WorkflowStep]` | `:220` | ✓ slice |
| `SequentialWorkflow::instructions` | `&[InternalPromptMessage]` | `:225` | ✓ slice |
| `SequentialWorkflow::has_task_support` | `bool` | `:200` | excluded by D-11 |
| `SequentialWorkflow::validate` | `Result<(), WorkflowError>` | `:239` | — |
| `WorkflowStep::name` | `&StepName` | `workflow_step.rs:294` | ✓ |
| `WorkflowStep::tool` | `Option<&ToolHandle>` | `:299` | ✓ |
| `WorkflowStep::is_resource_only` | `bool` | `:304` | ✓ |
| `WorkflowStep::arguments` | `&IndexMap<ArgName, DataSource>` | `:309` | ✓ insertion order |
| `WorkflowStep::binding` | `Option<&BindingName>` | `:314` | ✓ |
| `WorkflowStep::guidance` | `Option<&str>` | `:319` | ✓ |
| `WorkflowStep::resources` | `&[ResourceHandle]` | `:324` | ✓ slice |
| `WorkflowStep::template_bindings` | **`&HashMap<String, DataSource>`** | `:329` | **✗ — must be sorted** |
| `WorkflowStep::is_retryable` | `bool` | `:359` | excluded by D-11 |

`ArgumentSpec` (`sequential.rs:40-50`) has **all-public fields**: `pub description: String`, `pub required: bool`, `pub arg_type: Option<PromptArgumentType>`. The `arg_type` field is the "typed_argument schema information" left to discretion — it is available for the Inputs section at zero cost.

`SequentialWorkflow` derives `Clone, Debug` (`sequential.rs:16`); internally `arguments: IndexMap` (with the in-source comment *"`IndexMap` for deterministic iteration"*, `:27-28`), `steps: SmallVec<[WorkflowStep; 3]>`, `instructions: SmallVec<[InternalPromptMessage; 3]>` — all ordered.

**`instructions()` and the `PromptContent` variants.** `instructions()` returns `&[InternalPromptMessage]`; each is `{ pub role: Role, pub content: PromptContent }` (`prompt_content.rs:41-46`). `PromptContent` is **`#[non_exhaustive]`** (`prompt_content.rs:13`) with six variants:

```rust
pub enum PromptContent {
    Text(String),
    Image { data: String, mime_type: String },
    ResourceUri(String),
    ToolHandle(ToolHandle),
    ResourceHandle(ResourceHandle),
    Multi(SmallVec<[Box<Self>; 3]>),
}
```

`#[non_exhaustive]` means the renderer's `match` **requires** a `_ =>` arm regardless — that is a compiler constraint, not a style choice. Recommended rendering (mirrors what `PromptContent::to_protocol` at `conversion.rs:49-107` already does at the wire edge, so the two stay conceptually aligned):

| Variant | Render |
|---|---|
| `Text(t)` | `t` verbatim (this is spike 009's `{other:?}` bug fixed) |
| `Image { mime_type, .. }` | `` `(image content: {mime_type})` `` — **never** the base64 `data`: it is unbounded and would blow the SEP-2640 16 MiB limit and the digest |
| `ResourceUri(u)` | `` Read the resource `{u}`. `` |
| `ToolHandle(h)` | `` Uses tool `{h.name()}`. `` — name only, per D-12 (no schema) |
| `ResourceHandle(h)` | `` Read the resource `{h.uri()}`. `` |
| `Multi(parts)` | recurse, join with `"\n\n"` (matches `conversion.rs:97-106`) |
| `_` (future) | one stable literal, e.g. `` `(unsupported instruction content)` `` — **must be a constant string**, never `{:?}`, or a future variant silently changes the golden digest |

`Role` (`src/types/content.rs:807-814`) has `User`, `Assistant`, `System` with a `Display` impl (`:816-822`) emitting `"user"`/`"assistant"`/`"system"`. Whether the projected Context section labels the role is a render choice; if it does, use `Display`, not `Debug`.

**`template_bindings()` sort key.** Sort by the `String` key (the template variable name) with `sort_unstable()` / `BTreeMap` collection. Keys are unique by `HashMap` construction, so `sort_unstable` is total and `sort` vs `sort_unstable` is immaterial. Byte-order (`Ord for String`) is the right key because it is locale-independent — a locale-aware collation would break byte-equality across machines. This is the single determinism landmine flagged in CONTEXT `<specifics>` and it is confirmed exactly as stated.

**`DataSource::Constant(Value)` determinism — `preserve_order` IS enabled.** `Cargo.toml:119`:

```toml
serde_json = { version = "1.0", features = ["raw_value", "preserve_order"] }
```

With `preserve_order`, `serde_json::Map` is `IndexMap`-backed, so `to_string`/`to_string_pretty` emit keys in **construction/parse order** — deterministic for a given workflow definition, and stable across re-derivations of the same `SequentialWorkflow` value. Two caveats to write into the plan:

1. **Cargo feature unification makes this a workspace-wide fact, not a pmcp-local one** — any dependency could in principle also enable it (they all get the same `serde_json`), but nothing can *disable* it. Safe.
2. `preserve_order` means the render mirrors the author's key order, **not** a canonical order. Two workflows that are semantically identical but wrote `{"a":1,"b":2}` vs `{"b":2,"a":1}` render differently. That is correct for SC-2 (same workflow ⇒ same bytes) but means the golden file is sensitive to the fixture's literal key order. If the planner prefers canonical output, sort explicitly — do not rely on `preserve_order` for canonicality it does not provide.

Recommendation: render `Constant` as `` use the constant value `{}` `` with `serde_json::to_string(v)` (compact, single-line — multi-line pretty JSON inside a markdown bullet is a readability and diff hazard). `DataSource` is also `#[non_exhaustive]` (`data_source.rs:9`), so its `match` needs a `_` arm too, with the same constant-string rule.

### Q3. Slugification (D-15, SC-1)

**What Phase 125 actually enforces — quoted from `src/server/skills/…` (`skills.rs` today):**

`validate_names` (`skills.rs:1444-1469`) is the *only* name rule in the SDK:

```rust
fn validate_names(artifacts: &[SkillBuildArtifact]) -> Result<()> {
    let mut offenders: Vec<String> = Vec::new();
    for artifact in artifacts {
        let Some(name) = artifact
            .frontmatter
            .as_ref()
            .and_then(|fm| fm.get("name"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        if name != artifact.uri_segment {
            offenders.push(format!(
                "{} (frontmatter name '{}', URI segment '{}')",
                artifact.uri, name, artifact.uri_segment
            ));
        }
    }
    …
    Err(Error::validation(format!(
        "Skills: frontmatter `name` must equal the final segment of the skill's URI path: [{}]",
        offenders.join(", ")
    )))
}
```

`uri_segment` is `final_path_segment(skill.resolved_path())` (`skills.rs:1218`, helper at `:1217-1219`), and `resolved_path` is:

```rust
pub(crate) fn resolved_path(&self) -> &str {
    self.path.as_deref().unwrap_or(&self.name)      // skills.rs:421-423
}
pub(crate) fn skill_md_uri(&self) -> String {
    format!("skill://{}/SKILL.md", self.resolved_path())   // :425-427
}
```

**Therefore SC-1 holds by construction and needs no new machinery:** if the projection calls `Skill::new(slug, body)` with the frontmatter `name: {slug}` and **never** calls `.with_path(...)`, then `resolved_path() == slug`, `final_path_segment(slug) == slug` (no `/` in a legal slug), and `frontmatter.name == slug`. `validate_names` is satisfied identically.

**There is NO character-set validation on skill names in the SDK.** `validate_names` checks identity only. So agentskills legality is entirely the projection's responsibility.

**The authoritative character set** `[CITED: https://agentskills.io/specification#name-field]` — the `name` field:

> * Must be 1-64 characters
> * May only contain unicode lowercase alphanumeric characters (`a-z`, `0-9`) and hyphens (`-`)
> * Must not start or end with a hyphen (`-`)
> * Must not contain consecutive hyphens (`--`)
> * Must match the parent directory name

and the frontmatter table row: *"`name` | Yes | Max 64 characters. Lowercase letters, numbers, and hyphens only. Must not start or end with a hyphen."*

SEP-2640 delegates entirely: *"The final `<skill-path>` segment, being the skill `name`, MUST satisfy the Agent Skills specification's naming rules"* `[CITED: seps/2640-skills-extension.md:68]`, and *"The final segment of `<skill-path>` MUST equal the skill's `name` as declared in its `SKILL.md` frontmatter"* `[CITED: :63]`.

**Concrete normalization algorithm** (satisfies both rule sets; deterministic; total):

```
fn slugify(name: &str) -> Option<String>          // None ⇒ nothing legal survived
 1. lower = name.to_lowercase()                   // Unicode-aware; harmless, step 2 filters
 2. mapped = lower.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
 3. joined = mapped.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-")
    // steps 2-3 = spike 009 main.rs:68-77 — kills leading/trailing/consecutive hyphens
 4. if joined.is_empty() { return None }
 5. if joined.len() > 64 {
        joined.truncate(64);                      // ASCII-only after step 2 ⇒ byte==char, no panic
        while joined.ends_with('-') { joined.pop(); }
        if joined.is_empty() { return None }      // unreachable: char 1 is alnum, but keep it total
    }
 6. Some(joined)
```

Post-conditions to assert as a proptest invariant (SC-2 / D-15): `1 <= len <= 64`; every char in `[a-z0-9-]`; no leading/trailing `-`; no `--`. Note step 5's truncate is panic-free **only because** step 2 has already reduced the string to ASCII — `String::truncate` panics on a non-char-boundary index. Write that reason into a code comment; it is not obvious.

**The `workflow-{8 hex}` fallback (D-15).** When `slugify` returns `None`:

- **Hash function:** reuse the crate-private `sha256_digest_hex` at `skills.rs:1801-1817`. **Do not** add a second digest implementation, and **do not** copy the spike-era `format!("{:x}", …)` — that form does not compile here. The reason is documented at the call site (`skills.rs:1790-1793`):

  > *"The whole-value `{:x}` formatter does NOT compile on this workspace's stack: MEASURED at plan time, there is no `LowerHex` impl anywhere in `sha2-0.11.0`, `digest-0.11.2` or `crypto-common-0.2.2`."*

  `Cargo.toml:149` confirms `sha2 = "0.11"`.
- **Input:** the **original**, un-normalized `wf.name()` as UTF-8 bytes. Hashing the empty normalized string would collide every failing workflow onto one name — which `Skills::into_handler`'s duplicate-URI check would then reject, turning a warn-path into a hard build failure. Hash the original.
- **Format:** `sha256_digest_hex(wf.name().as_bytes())` returns `"sha256:" + 64 lowercase hex`. Take 8 chars **after** the prefix: `&digest["sha256:".len()..][..8]`, giving `format!("workflow-{}", eight)`. The result is 17 chars, `[a-z0-9-]`, one internal hyphen — legal by construction, and legal *unconditionally*, which is why nothing needs to re-validate it.

**Duplicate-URI rejection still returns `Err(Error::Validation)` — confirmed.** `Skills::into_handler` (`skills.rs:1118`) → `build_handler` (`:1137`) → `:1189`:

```rust
return Err(Error::validation(msg));
```

where `msg` starts `"Skills::into_handler: duplicate URI(s):"` and lists `SKILL.md=[…]`, `references=[…]`, and (Phase 125 addition) `a reference collides with another skill's SKILL.md=[…]`. `Skills::entries_with_diagnostics` runs the twin `validate_unique_uris` (`:1385`) so the listing and the handler cannot disagree. **No new collision machinery is needed**, exactly as D-15 says.

**WR-03 caveat the plan must respect.** `ServerBuilder::skills`/`.skill` are infallible and `#[must_use]`; the failure surfaces as a **`panic!`** at `src/server/builder.rs:1501` inside `finalize_skills_resources`. That finding is still open (`.planning/phases/125-…/deferred-items.md:128`). A projected skill cannot trip `validate_names` (identity holds by construction), so Phase 126 does not *worsen* WR-03 — but the `s56` example and the docs should register projected skills via `.try_skills(...)` where a duplicate is plausible.

### Q4. The D-04 prompt prepend

**See CONFLICT 1 above for the blocking finding.** Mechanics, assuming the conflict is resolved:

**`prompt_handler.rs` seam.** `WorkflowPromptHandler::handle` (`:750-957`, `#[async_trait] impl PromptHandler`):

```rust
let mut messages = Vec::new();                        // :767
let mut execution_context = ExecutionContext::new();  // :768
// 1️⃣ User Intent Message
messages.push(self.create_user_intent(&args));        // :771
// 2️⃣ Assistant Plan Message (list all workflow steps)
messages.push(self.create_assistant_plan()?);         // :774
```

The prepend goes immediately before `:771`. There are **five** early-return / `break` exits after that point (`:821`, `:852`, `:880`, `:934`, `:942`) that each build a `GetPromptResult` from the accumulated `messages` — inserting at `:770` means the prepended body survives every one of them, which is what "message [0]" requires.

**`task_prompt_handler.rs` is a SEPARATE code path, not a delegation.** `TaskWorkflowPromptHandler` (`:201-208`) wraps `inner: WorkflowPromptHandler` (`:203`). Its `impl PromptHandler` (`:632`) `handle` (`:643`) has two branches:

- one calls `self.inner.handle(args, extra).await?` at `:677` — inherits the prepend for free;
- the other **re-builds the message list itself** at `:691-692`:
  ```rust
  messages.push(self.inner.create_user_intent(&args));
  messages.push(self.inner.create_assistant_plan()?);
  ```
  and then re-implements the per-step loop (guidance at `:717`, template bindings at `:723`, param resolution via `self.inner.…` at `:779`/`:795`).

So a prepend added only to `prompt_handler.rs` covers one of the two task branches and misses the other. **Recommend one plan, both files**, with the body computed once by a shared `pub(crate) fn` on `WorkflowPromptHandler` (e.g. `fn projected_prepend(&self) -> Option<PromptMessage>`) that `task_prompt_handler.rs` calls through `self.inner` — the same delegation shape `:691-692` already uses. Two plans would leave a window where `has_task_support(true)` workflows behave differently from the others, which is precisely the drift this phase exists to prevent.

**Carrying the opt-in without breaking the public signature.** `WorkflowPromptHandler::new(workflow, tools, tool_handlers, resource_handler)` (`:129-142`) and `::with_middleware_executor(workflow, tools, middleware_executor, resource_handler)` (`:155-…`) are both public and both must keep their arity. Add a private field plus a `#[must_use]` chainable setter — this is the module's own idiom:

```rust
pub struct WorkflowPromptHandler {
    workflow: SequentialWorkflow,                       // :94
    tools: HashMap<Arc<str>, ToolInfo>,                 // :96   (workflow::conversion::ToolInfo)
    middleware_executor: Option<Arc<dyn MiddlewareExecutor>>,  // :98
    tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>>,    // :100
    resource_handler: Option<Arc<dyn ResourceHandler>>,        // :102
    // NEW:
    #[cfg(feature = "skills")]
    prepend_projected_skill: bool,                      // default false
}
```

with `#[cfg(feature = "skills")] #[must_use] pub fn with_projected_skill_prepend(mut self, on: bool) -> Self`. A `#[cfg]`'d **field** is fine here — the struct's `Debug` impl is hand-written (`:105-118`), so no derive needs updating, and every constructor is in-file. Note `TaskWorkflowPromptHandler::new(inner, task_router, workflow)` (`:227-…`) takes the already-configured `inner`, so the flag rides along with no signature change there either.

**Reaching it from `ServerCoreBuilder::prompt_workflow`** (`src/server/builder.rs:1208-…`): the handler is constructed at `:1245-1250`; a builder-level opt-in would add a `ServerCoreBuilder` field consulted there. That is additive and optional — the planner may scope the flag to direct `WorkflowPromptHandler` construction only, but then `s56` must construct the handler directly (which spike 009 already does: `main.rs:328`).

### Q5. `as_prompt_text()` (SC-5) — **IT EXISTS**

`Skill::as_prompt_text(&self) -> String` at **`src/server/skills.rs:451-467`**, public, on `Skill`:

```rust
pub fn as_prompt_text(&self) -> String {
    let mut out = String::new();
    out.push_str(&self.body);
    if !self.body.ends_with('\n') {
        out.push('\n');
    }
    for r in &self.references {
        out.push_str("\n--- ");
        out.push_str(&r.relative_path);
        out.push_str(" ---\n");
        out.push_str(&r.body);
        if !r.body.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}
```

Its rustdoc (`:437-450`) calls it *"the load-bearing dual-surface invariant"*.

**SC-5's `as_prompt_text() == body` is therefore satisfiable exactly, under two conditions the renderer must guarantee:**

1. **The projected skill registers ZERO `SkillReference`s.** With one reference, `as_prompt_text()` appends `"\n--- {path} ---\n{body}\n"` and equality fails. D-11 keeps everything in one body, so this holds — but it is a *constraint on the renderer*, not a free property, and the test must assert `skill.references().count() == 0` alongside the equality so a future reference addition fails loudly rather than mysteriously.
2. **The rendered body ends with `'\n'`.** Otherwise `as_prompt_text()` appends one and returns `body + "\n"` ≠ `body`. Make "the body always terminates in exactly one `\n`" an explicit renderer post-condition and a proptest invariant.

This is *not* a load-bearing ambiguity — SC-5 binds to a real shipped method. `Skill::body()` is at `:405-407`.

### Q6. The SC-6 gate warning (D-07..D-10)

**`ToolAnnotations` confirmed** — `src/types/tools.rs:20-53`, `#[derive(Debug, Clone, Serialize, Deserialize, Default)] #[non_exhaustive] #[serde(rename_all = "camelCase")]`:

```rust
pub struct ToolAnnotations {
    pub title: Option<String>,              // :23
    pub read_only_hint: Option<bool>,       // :27
    pub destructive_hint: Option<bool>,     // :31
    pub idempotent_hint: Option<bool>,      // :35
    pub open_world_hint: Option<bool>,      // :39
    pub output_type_name: Option<String>,   // :52  (PMCP extension, "pmcp:outputTypeName")
}
```

All fields `Option<bool>` as D-07/D-08 assume; `#[non_exhaustive]` so never use struct-literal syntax — use `ToolAnnotations::new()` (`:57`) + `with_read_only` (`:70`) / `with_destructive` (`:80`).

**How a caller obtains the tool map — the type question, answered.** There are **two** `ToolInfo` types in this crate and only one carries annotations:

| Type | Fields | Has `annotations`? |
|---|---|---|
| `crate::server::workflow::conversion::ToolInfo` (`conversion.rs:17-24`) | `name: String`, `description: String`, `input_schema: Value` | **NO** |
| `crate::types::ToolInfo` (`src/types/tools.rs:199-230`) | `name`, `title`, `description: Option<String>`, `input_schema`, `output_schema`, **`annotations: Option<ToolAnnotations>`** (`:219`), `icons`, `_meta`, `execution` | **YES** |

`ServerCoreBuilder::prompt_workflow` (`builder.rs:1223-1232`) **drops the annotations** when it converts:

```rust
for (name, info) in &self.tool_infos {
    tool_registry.insert(
        Arc::from(name.as_str()),
        workflow::conversion::ToolInfo {
            name: info.name.clone(),
            description: info.description.clone().unwrap_or_default(),
            input_schema: info.input_schema.clone(),
        },
    );
}
```

So `WorkflowPromptHandler.tools` is **useless for SC-6** — this is why D-07 makes the warning a builder-path capability rather than something `as_skill()` can do.

`ServerCoreBuilder.tool_infos: HashMap<String, ToolInfo>` (`builder.rs:73`) is **private**, so the caller cannot borrow it from the builder either.

**Realistic caller sources for the annotations, in order of practicality:**

1. The caller owns the tool definitions — they wrote `TypedToolWithOutput::with_annotations(ToolAnnotations::new().with_destructive(true))` (`src/server/typed_tool.rs:138`, `:356`, `:674`) — and hands the projection the same values.
2. `ToolHandler::metadata() -> Option<crate::types::ToolInfo>` (`src/server/mod.rs:366`) — the trait method that produces annotations for `tools/list`; a caller holding `Arc<dyn ToolHandler>`s can map over them.
3. A `tools/list` response: `ListToolsResult.tools: Vec<crate::types::ToolInfo>` — the natural shape for a client-side projection.

**Recommended `with_tools` signature:** `pub fn with_tools(self, tools: impl IntoIterator<Item = crate::types::ToolInfo>) -> Self`, internally collapsing to `HashMap<String, Option<ToolAnnotations>>` keyed by `ToolInfo::name`. `IntoIterator` accepts `Vec<ToolInfo>` (source 3) and `&[ToolInfo]`-via-`.iter().cloned()` alike; a `&HashMap<String, ToolInfo>` parameter would force sources 1 and 3 to build a map they do not otherwise need. Avoid taking `workflow::conversion::ToolInfo` — it structurally cannot carry the signal.

**`tracing::warn!` precedent, verbatim** (`src/server/skills.rs:1008-1018`):

```rust
pub fn entries(&self) -> Result<Vec<SkillEntry>> {
    let (entries, diagnostics) = self.entries_with_diagnostics()?;
    for diagnostic in &diagnostics {
        tracing::warn!(
            target: "mcp.skills",
            uri = %diagnostic.uri(),
            "{}",
            diagnostic.message()
        );
    }
    Ok(entries)
}
```

The same shape appears at `src/server/builder.rs:1503-1508`. Note the two-part idiom D-10 mirrors: a crate-private `*_with_diagnostics` returning `(product, Vec<Diagnostic>)` that tests assert on, wrapped by the public infallible method that logs them. `SkillDiagnostic` (`skills.rs:617`) is deliberately **crate-private** — its rustdoc (`:606-616`) says *"Crate-private on purpose: it adds no public API surface while making every warn path directly assertable from the in-module test block."* D-10 requires `build()` to **return** the warnings, so `ProjectionWarning` must be **public** — a deliberate departure from the `SkillDiagnostic` precedent that the planner should note in the type's rustdoc so a future reader does not "fix" it back to private.

**Warning key (D-09) restated against the live types:** for each `step` in `wf.steps()` where `step.guidance().is_some()` and `step.tool()` is `Some(h)`, look up `h.name()`:
- `annotations.read_only_hint == Some(false)` **or** `annotations.destructive_hint == Some(true)` ⇒ **warn** (D-08);
- tool present in the map but `annotations == None` ⇒ **"unverifiable" note** (D-08's distinct diagnostic);
- tool absent from the map entirely ⇒ also "unverifiable" (same class — the check could not be performed);
- `with_tools` never called ⇒ no diagnostics at all (D-07's documented consequence).

Resource-only steps (`step.tool() == None`) can have no annotations and execute no tool: exclude them from the check.

### Q7. Testing (ALWAYS requirements + SC-2/3/4)

#### Fuzz

- **Location:** `fuzz/fuzz_targets/*.rs` (26 targets present). The nearest sibling is `fuzz/fuzz_targets/fuzz_skill_entry.rs` (8.4 K, Phase 125).
- **Registration:** a `[[bin]]` stanza in `fuzz/Cargo.toml` — `fuzz_skill_entry` at `:333-338`:
  ```toml
  [[bin]]
  name = "fuzz_skill_entry"
  path = "fuzz_targets/fuzz_skill_entry.rs"
  test = false
  doc = false
  bench = false
  ```
  `fuzz/Cargo.toml:60` already enables `skills` on the pmcp dep: `features = ["oauth", "streamable-http", "fuzzing", "validation", "skills"]` — **no manifest feature change is needed** for a projection fuzz target.
- **`make test-fuzz` / `make validate-always` run NOTHING on stable — VERIFIED by falsification.** `Makefile:786-797`:
  ```make
  cd fuzz && $(CARGO) fuzz list | while read target; do \
      timeout 30s $(CARGO) fuzz run $$target || echo "$(YELLOW)Fuzz target $$target completed$(NC)"; \
  done;
  ```
  The `|| echo` swallows every failure. Measured on this machine (`rustc 1.98.0 (88d9e12ae 2026-08-18)`, stable, active default):
  ```
  $ cd fuzz && cargo fuzz run fuzz_skill_entry -- -runs=1
  error: failed to run `rustc` to learn about target-specific information
    --- stderr
    error: the option `Z` is only accepted on the nightly compiler
    help: consider switching to a nightly toolchain: `rustup default nightly`
    error: 1 nightly option were parsed
  Error: failed to build fuzz script: … "-Zsanitizer=address" … --bin fuzz_skill_entry
  ```
  `cargo fuzz list` itself works (exit 0; it only reads the manifest), which is why the loop iterates and reports "completed" for every target. **The recorded project memory is correct.** `cargo-fuzz` is installed at `/Users/guy/.cargo/bin/cargo-fuzz` and `nightly-aarch64-apple-darwin` **is** installed (`rustup toolchain list`), so the working invocation is `cargo +nightly fuzz run <target> -- -runs=100000 -max_total_time=30`.
- **Consequence for the plan:** a fuzz target's *existence and registration* must be asserted by a normal test, not by `make test-fuzz`. The precedent already exists — `tests/skills_routing.rs:1431` `fuzz_skill_entry_is_registered_and_scheduled` checks (1) the source file exists and contains `fuzz_target!`, (2) a `[[bin]]` stanza names it with a `path` that resolves, and (3) it is on the CI fuzz-matrix schedule, with an explicit anti-vacuity assertion (`:1459-1464`) that `fuzz/Cargo.toml` contains `[[bin]]` at all. **Copy that test wholesale for the new target.** Also run `cargo +nightly fuzz run <target> -- -runs=…` once by hand and paste the output into the plan's evidence — a plan that claims fuzz coverage on the strength of `make test-fuzz` is claiming nothing.

#### Property tests

- **Harness:** `proptest` (`Cargo.toml:245`, `proptest = "1.7"`). `quickcheck`/`quickcheck_macros` are also dev-deps (`:246-247`) but the skills/workflow area uses **proptest**.
- **Existing files to follow:** `src/server/skills.rs:2337-2410` — strategies `skill_strategy()`, `skills_strategy_with_refs()` and `proptest! { }` blocks at `:2381`, `:2399`, inside the in-module `#[cfg(test)] mod tests`; and `tests/skills_integration.rs:477`, `:511` — `proptest! { }` blocks at integration level, including `prop_assert_eq!(sep_2640, prompt)` at `:504` (the dual-surface equality property, the direct ancestor of SC-5).
- **`make test-property` does NOT reach skills.** `Makefile:781-784`:
  ```make
  PROPTEST_CASES=1000 … $(CARGO) test --features "full" -- --ignored property_
  ```
  Two independent reasons it misses this module: `--features "full"` excludes `skills` (the module is `#[cfg]`'d out entirely), and `--ignored` runs only `#[ignore]`d tests. The skills proptests are neither. **Do not** add `#[ignore]` + a `property_` prefix hoping to be picked up — that would make them run under a feature set where the module does not exist, i.e. zero tests, exit 0. Put them where the existing ones are, and let `make test-skills` selectors 1 and 3 run them.

#### Golden file (D-14)

- **`insta` is a dev-dependency (`Cargo.toml:253`, `insta = { version = "1.43", features = ["json","redactions"] }`) but is used NOWHERE** — `grep -rln "insta::" tests/ src/` returns nothing. Do not introduce the first `insta` usage in this phase; it adds a snapshot-review workflow (`cargo insta review`) that no other test in the tree uses and that CI has no story for.
- **The in-repo pattern is a vendored file + `include_str!`.** Precedent: `tests/v2_tasks_shapes.rs:66`
  ```rust
  const EXT_TASKS_SCHEMA_JSON: &str = include_str!("../schema/vendored/ext-tasks/schema.json");
  ```
  `include_str!` registers a cargo rebuild dependency, so editing the golden re-runs the test — which is exactly the D-14 tripwire behaviour.
- **Counter-precedent worth knowing:** `tests/v2_tasks_tripwires.rs:97-99` argues for runtime `read_to_string` instead, because `include_str!` bakes bytes at compile time and therefore cannot prove the file *exists on disk at test time*. That reasoning applies to "assert a file is present" tests, not to golden comparison. For D-14, `include_str!` is correct.
- **Recommended placement:** `tests/golden/workflow_skill_projection.md` (new directory; `tests/` currently has `tests/common/`, `tests/integration/`), consumed from `tests/skills_integration.rs` via `include_str!("golden/workflow_skill_projection.md")`. Placing the consumer in `skills_integration.rs` (rather than a new test file) means **no new Makefile selector is needed** — see below.
- **Failure message must tell the operator what to do.** Follow the house style (e.g. `Makefile:1030`, `skills.rs:1466`): name the file, say "if this change is intentional, update the golden AND add a CHANGELOG entry — a digest-pinning consumer must re-pin (D-14)".

#### SC-4: "on the wire, through the real handler" — two existing homes, no socket needed for the byte-identity half

- **`tests/skills_integration.rs`** (`#![cfg(all(feature = "skills", not(target_arch = "wasm32")))]` at `:31`) drives the `ResourceHandler` **in-process**, exactly the `CONVENTIONS.md` pattern (*"call `pmcp::server::ToolHandler::handle(&handler, args, extra).await` directly"*, `.planning/spikes/CONVENTIONS.md:102-106`; `RequestHandlerExtra::default()` is fine, `:76-79`):
  ```rust
  async fn build_handler() -> Arc<dyn ResourceHandler> { … .into_handler() … }   // :109-113
  let main = handler.read(&main_uri, extra.clone()).await.unwrap();              // :127
  let handler = Skills::new().add(skill.clone()).into_handler().unwrap();        // :293
  ```
- **`tests/skills_routing.rs`** drives **real bytes over a loopback socket** through the real `StreamableHttpServer` — its header says so (`:1-10`), and it has `skills_fixture_server(registry: Skills) -> Server` plus `one_skill_registry()` fixtures ready to reuse. SC-4's "carrying a conforming SEP-2640 entry" belongs here: assert `skills/list` returns the projected skill's entry with verbatim frontmatter and a complete `{uri, digest: "sha256:"+64hex, size}` manifest.
- **`SkillEntry` accessors for the assertions** (`skills.rs:583-604`): `uri() -> &str`, `frontmatter() -> &serde_json::Value`, `resources() -> &[SkillResourceRef]`; and `SkillResourceRef` (`:548-565`): `uri()`, `digest()`, `size()`.

#### Which test files `make test-skills` actually reaches

`Makefile:1021-1086` runs **exactly four** guarded selectors, each with a zero-count guard and a named-binary check via `scripts/named-test-binary-count.awk`:

```make
.PHONY: test-skills
test-skills:
	@echo "$(BLUE)Running the skills module's tests (features: $(SKILLS_FEATURES))...$(NC)"
	@out=$$(RUSTFLAGS= … $(CARGO) test -p pmcp --features "$(SKILLS_FEATURES)" --lib skills -- --test-threads=1 2>&1); \
	…
	if [ "$$ran" -eq 0 ]; then \
		echo "$(RED)✗ selector 1 (--lib skills) reported 0 tests. …$(NC)"; \
		exit 1; \
	fi; \
	n=$$(printf '%s\n' "$$out" | awk -v want="src/lib.rs" -f scripts/named-test-binary-count.awk); \
	…
```

| # | Selector | Reaches |
|---|---|---|
| 1 | `--lib skills` (`:1024`) | in-crate unit + proptests whose **full test path contains the substring `skills`** |
| 2 | `--doc skills` (`:1042`) | doctests on items whose path contains `skills` |
| 3 | `--test skills_integration` (`:1054`) | `tests/skills_integration.rs` only |
| 4 | `--test skills_routing` (`:1070`) | `tests/skills_routing.rs` only |

with `SKILLS_FEATURES := skills,streamable-http,http-client,testing` (`Makefile:954`) — **not** `full`.

**Rule for the plan: add NO new integration test file.** A `tests/skills_projection.rs` would be invisible to all four selectors and would need a fifth ~18-line guarded selector added to the Makefile. Put integration tests in `skills_integration.rs` (byte-identity, golden, in-process handler reads) and `skills_routing.rs` (wire proofs, fuzz-registration tripwire). Put unit + proptests in `src/server/skills/projection.rs`'s `mod tests`.

`make test-skills` is chained into `quality-gate` at `Makefile:1910`, with `lint-skills` at `:1913` — both with in-Makefile comments explaining that `test-all` and `lint` pin `--features "full"` and therefore cannot reach this module.

#### Example

`make test-examples` → `scripts/run-example-builds.sh:150` runs `cargo build -p "$label" --all-features --examples`. `--all-features` includes `skills`, so the new example is **built** by the quality gate (s44 proves the path works). It is not *run* by the gate — the ALWAYS `cargo run --example` requirement is a manual/plan-verify step.

### Q8. Feature gating

**`skills` is absent from `default`, `full`, and `full-v2` — confirmed.** `Cargo.toml:279-295`:

```toml
default = ["logging", "v1-compat"]
full = ["websocket", "http", "streamable-http", "sse", "validation", "resource-watcher", "rayon", "schema-generation", "jwt-auth", "composition", "mcp-apps", "http-client", "logging", "macros", "testing", "v1-compat"]
…
full-v2 = ["websocket", "http", "streamable-http", "sse", "validation", "resource-watcher", "rayon", "schema-generation", "jwt-auth", "composition", "mcp-apps", "http-client", "logging", "macros", "testing"]
```

and `Cargo.toml:318`: `skills = ["dep:serde_yaml"]`, with the in-manifest comment at `:313-317` stating the exclusion is deliberate and pointing at the tripwire.

**`tests/v1_severability_tripwire.rs` exists (71 K).** It **derives** all three lists from `Cargo.toml` at test time rather than re-enumerating them (`:29` *"Why the scope is DERIVED, not enumerated"*; `MANIFEST` const at `:49`). Key assertions:

- `full_and_full_v2_differ_by_exactly_v1_compat` (`:143`) — `full \ full-v2` must be **exactly** `{v1-compat}`, and `full-v2` must be a strict subset of `full` (`:166`);
- anti-vacuity floors `MIN_FULL_ENTRIES` / `MIN_FULL_V2_ENTRIES` (`:66-67`) so a parse failure cannot pass as an empty set;
- a presence check that `full`, `full-v2` and `default` all exist (`:95-98`).

**What a plan must avoid to keep it green: do not add `skills` (or anything else) to `full` or `full-v2`.** Adding an `[[example]]` stanza does not touch `[features]` and is safe. The tripwire also has no opinion about `[[example]]` entries.

**`#[cfg(feature = "skills")]` on the ungated `SequentialWorkflow`:**

- **No in-module precedent** — `grep -rn "#\[cfg(feature" src/server/workflow/` returns **0 matches**. `as_skill()` would be the first.
- **Strong in-crate precedent** for exactly this shape (a `skills`-gated inherent method on an otherwise-ungated type): `src/server/mod.rs:4683-4687` and `:4704-…`:
  ```rust
  #[cfg(feature = "skills")]
  #[must_use]
  pub fn skill(self, skill: skills::Skill) -> Self {
      self.skills(skills::Skills::new().add(skill))
  }
  ```
  Also `:3334`, `:3437`, `:1758`, `:1789`, `:500`, `:5562`, `:5637`.
- **wasm32 is a non-issue.** `pub mod workflow;` is itself `#[cfg(not(target_arch = "wasm32"))]` (`src/server/mod.rs:164-165`), and `skills` is `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` (`:194-195`). A `#[cfg(feature = "skills")]` item inside `workflow` is therefore *implicitly* also non-wasm — the two gates compose to `all(feature="skills", not(wasm32))`, exactly matching `skills`'s own gate. There is no configuration in which `as_skill()` compiles while `skills` does not exist. **No `all(...)` cfg is needed on `as_skill()`; a bare `#[cfg(feature = "skills")]` is correct and matches the `mod.rs` precedent.**
- **Doctest gating.** With `#![warn(missing_docs)]` (`src/lib.rs:24-29`) plus `doc-check` running `RUSTDOCFLAGS="-D warnings"` with `skills` on (`Makefile:1506-1507`), `as_skill()` needs a doctest, and that doctest must self-gate:
  ```rust
  /// ```rust
  /// # #[cfg(feature = "skills")] {
  /// use pmcp::server::workflow::SequentialWorkflow;
  /// let wf = SequentialWorkflow::new("refund_flow", "Process a refund");
  /// assert_eq!(wf.as_skill().name(), "refund-flow");
  /// # }
  /// ```
  ```
  (precedent: `skills.rs:528`, `:970`, `src/server/mod.rs:4670-4681`). **But note:** a doctest on a `workflow::` path does **not** contain the substring `skills`, so `make test-skills` selector 2 (`--doc skills`) will **not run it**. It runs under `cargo test --doc --all-features` (CI `ci.yml:113`) and under `make quality-gate`'s `test-all` → `test-doc` only if that leg's features include `skills` — they do not (`--features "full"`). **Verify this doctest explicitly** with `cargo test -p pmcp --features "skills,full" --doc sequential -- --test-threads=1`, and say so in the plan's verify block.

### Q9. The `s45` example (D-16) — number must change to `s56`

**`Cargo.toml:700-717` verbatim:**

```toml
path = "examples/c09_client_list_all.rs"
required-features = ["full"]

[[example]]
name = "s44_server_skills"
path = "examples/s44_server_skills.rs"
required-features = ["skills", "full"]

[[example]]
name = "c10_client_skills"
path = "examples/c10_client_skills.rs"
required-features = ["skills", "full"]

[[example]]
name = "s45_tool_as_task_lifecycle"
path = "examples/s45_tool_as_task_lifecycle.rs"
required-features = ["full"]
```

**What `s44` is** (`examples/s44_server_skills.rs:1-25`): *"Server example: SEP-2640 Skills (Phase 80) — Demonstrates the three-tier skill registration pattern + the dual-surface bootstrap. Skills live under `examples/skills/` and are embedded via `include_str!` at compile time."* It prints (1) registered SKILL.md URIs, (2) the entry projection count + first entry's URI and digest, (3) `bootstrap_skill_and_prompt` registering both surfaces, (4) the dual-surface byte length. Run line in its own header: `cargo run --example s44_server_skills --features skills,full`. It uses `Server::builder()` deliberately, not `ServerCoreBuilder`. D-16's "leaves s44's scope as hand-authored skills" is accurate — s44 embeds authored SKILL.md files.

**Smallest runnable `s56` covering all four demands** (mirroring s44's assert-then-print style; `c10` is the precedent for *asserting* rather than merely printing — `125-VERIFICATION.md` calls that out as what made it verifiable):

1. Build a `SequentialWorkflow` with an argument, ≥2 steps, ≥1 `.with_guidance(...)` on a side-effecting step, and one workflow-level `.instruction(...)` — reuse spike 009's `build_workflow(true)` shape (`main.rs:170-…`, the `refund_flow` fixture): it exercises `PromptArg`, `StepOutput{field:None}`, `StepOutput{field:Some}`, guidance, and bindings in one object.
2. `let skill = wf.as_skill();` → print the body; `assert_eq!(skill.name(), "refund-flow")` (SC-1); `assert_eq!(skill.as_prompt_text(), skill.body())` (SC-5); `assert_eq!(wf.as_skill().body(), skill.body())` (SC-2).
3. Register through `Skills::new().add(skill.clone()).into_handler()?` and read `skill://refund-flow/SKILL.md` back, asserting byte-identity; print the first entry's digest from `Skills::…entries()?` (SC-4, in-process).
4. `SkillProjection::new(&wf).with_tools([...ToolInfo with ToolAnnotations::new().with_destructive(true)...]).build()?` → print the returned `ProjectionWarning`s and assert exactly one gate warning names the side-effecting step (SC-6).
5. Construct `WorkflowPromptHandler::new(wf, infos, handlers, None)` twice — with and without the D-04 flag — call `PromptHandler::handle` in-process, and assert the flagged transcript's message [0] text equals `skill.body()` while the unflagged transcript is unchanged.

**Cargo stanza to add** (place it after the `c10_client_skills` stanza, keeping the skills examples contiguous):

```toml
[[example]]
name = "s56_workflow_skill_projection"
path = "examples/s56_workflow_skill_projection.rs"
required-features = ["skills", "full"]
```

**Invocation:** `cargo run --example s56_workflow_skill_projection --features skills,full` — the same shape s44's header documents. `["skills", "full"]` is required (not just `skills`) because the handler/tool plumbing the example needs pulls in `full`'s features, matching s44/c10.

### Q10. Verify commands that actually work

Both Phase-125 commands **still resolve and pass** — measured this session:

| Command | Result |
|---|---|
| `cargo test -p pmcp --all-features --lib skills -- --test-threads=1` | `test result: ok. 99 passed; 0 failed; 0 ignored; 0 measured; 2083 filtered out; finished in 1.32s` |
| `cargo test --all-features --test skills_routing -- --test-threads=1` | `test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s` |

Cross-checked against the gate's own feature set: `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --test skills_routing -- --test-threads=1` also gives **20 passed** — `--all-features` and `SKILLS_FEATURES` select the same set here, so neither hides tests from the other. (`125-VERIFICATION.md` says "47 test functions" in that file; the runnable count is 20 under both feature sets — the higher figure counts helpers.)

**Recommended verify commands for Phase 126:**

| Layer | Command |
|---|---|
| Renderer unit + proptests (fast inner loop) | `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --lib skills::projection -- --test-threads=1` |
| All skills unit tests (gate-equivalent selector 1) | `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --lib skills -- --test-threads=1` |
| Golden + byte-identity + in-process handler | `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --test skills_integration -- --test-threads=1` |
| SC-4 wire proof | `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --test skills_routing -- --test-threads=1` |
| `as_skill()` doctest (**not covered by any gate leg — see Q8**) | `cargo test -p pmcp --features "skills,full" --doc sequential -- --test-threads=1` |
| D-04 prompt-transcript tests | `cargo test -p pmcp --features "skills,full" --test workflow_prompt_e2e_test -- --test-threads=1` *(existing file, `tests/workflow_prompt_e2e_test.rs`, 5.2 K — verify its `#![cfg]` header admits `skills` before relying on it; if not, put the D-04 tests in `src/server/workflow/prompt_handler.rs`'s `mod tests`, whose path contains neither "skills" nor a gate selector — see the warning below)* |
| Full gate leg | `make test-skills` |
| Lint leg | `make lint-skills` (`LINT_SKILLS_FEATURES := full,skills`, `Makefile:980`) |
| Example | `cargo run --example s56_workflow_skill_projection --features skills,full` |
| Fuzz (real, needs nightly) | `cd fuzz && cargo +nightly fuzz run <target> -- -runs=200000 -max_total_time=30` |
| Whole gate | `RUSTFLAGS="" make quality-gate` |

> ⚠ **`RUSTFLAGS=""`** — recorded project memory: `RUSTFLAGS` is exported in CI and absent locally, and `make test-skills` itself sets `RUSTFLAGS=` on each leg (`Makefile:1024` etc.). Prefix the local gate run to match.

> ⚠ **D-04 test placement is a reachability hazard.** Tests for the prompt prepend naturally live near `prompt_handler.rs`, but a test at `server::workflow::prompt_handler::tests::*` matches **no** `make test-skills` selector and, being `#[cfg(feature = "skills")]`-conditional, runs under **no** `--features "full"` leg either — i.e. it is dark in the whole quality gate. Two workable placements: (a) name the tests so the path substring `skills` appears is **not** possible (module path is fixed), so instead (b) put them in `tests/skills_integration.rs` or `tests/skills_routing.rs`, driving `WorkflowPromptHandler` in-process per `CONVENTIONS.md:102-106`. Option (b) is the recommendation.

**Two recorded pitfalls, both confirmed relevant:**

1. **`cargo test` aborts after the first failing target**, so a failure count from a multi-target run is a *lower bound*. `make test-skills` sidesteps this by running four separate invocations each with `if [ $$status -ne 0 ]; then exit $$status; fi` — which is why the gate is per-selector rather than one big command. Mirror that in plan verify blocks: one command per target, not one command with several `--test` flags.
2. **nextest's `test(/foo/)` selector silently selects zero tests** — the Makefile already records this at `:951` (*"neither is a `cargo nextest -E 'test(...)'`"*). nextest **is** used by `make test` (`Makefile:230`, `nextest run --features "full"` — which does not reach skills at all) and installed in CI. **Do not use nextest in any Phase-126 verify block**; if it is unavoidable, use `binary(<name>)`, never `test(/…/)`.

### Q11. Landmines and pitfalls

| # | Landmine | Evidence | Mitigation |
|---|---|---|---|
| 1 | **A monolithic renderer trips the PMAT CI gate.** The gate is at **zero** violations today (`pmat quality-gate --fail-on-violation --checks complexity` → `Total violations: 0`, `✅ Quality gate PASSED`, exit 0, PMAT 3.15.0 — measured). Every current violation from `pmat analyze complexity --max-cognitive 25` (21 of them) is in `tests/`; **none in `src/`.** | Measured this session | Decompose into ~7 helpers: `render_frontmatter`, `render_context`, `render_inputs`, `render_procedure`, `render_step`, `render_data_source`, `render_closing`. Each is a straight-line `push_str` sequence with ≤2 branches. `render_step` is the risk (tool / resource-only / bindings / guidance / template bindings) — split it further if needed. Verify locally with `pmat quality-gate --fail-on-violation --checks complexity` before pushing; `make quality-gate` does **not** run PMAT (CLAUDE.md D-07). |
| 2 | **`make lint` never sees this code; `make lint-skills` does.** `make lint` (`Makefile:157-…`) is `cargo clippy --features "full" --lib --tests` — `skills` off, module absent. `make lint-skills` (`:982-1019`) uses `LINT_SKILLS_FEATURES := full,skills`. | `Makefile:169`, `:980`, `:985` | Both are in `quality-gate` (`:1897`, `:1913`). The allow-list at `:985-1018` already includes `-A clippy::format_push_string` and `-A clippy::too_many_lines` — both of which a string-building renderer would otherwise trip under `-W clippy::pedantic`/`nursery`. Do **not** narrow that list. |
| 3 | **Zero-SATD gate is a literal grep.** `Makefile:1998`: `! grep -r "TODO\|FIXME\|HACK\|XXX" src/ --include="*.rs"`. | Measured | No placeholder comments, and no prose containing the word "HACK" in `src/` rustdoc either. |
| 4 | **`PromptContent` and `DataSource` are both `#[non_exhaustive]`.** A `_ =>` arm is mandatory. If it Debug-formats (spike 009's `{other:?}`), a future variant silently changes the digest and breaks every pinned consumer. | `prompt_content.rs:13`, `data_source.rs:9` | The catch-all must emit a **constant** string. Pin it with a test. |
| 5 | **`ToolAnnotations` and `ToolInfo` are `#[non_exhaustive]`** — no struct-literal syntax anywhere in tests or the example. | `src/types/tools.rs:18`; `CONVENTIONS.md:81-89` | Use `ToolAnnotations::new().with_destructive(true)` etc. |
| 6 | **`sha2` is 0.11: `format!("{:x}", digest)` does not compile.** | `Cargo.toml:149`; `skills.rs:1790-1793` | Reuse `super::sha256_digest_hex` (`skills.rs:1801`). Do not write a second digest fn. |
| 7 | **SC-5 fails silently if the projected skill gains a reference or loses its trailing newline.** | `skills.rs:451-467` | Assert `references().count() == 0` and `body().ends_with('\n')` alongside the equality. |
| 8 | **`WorkflowStep::template_bindings()` is a `HashMap` and D-11 renders it.** Unsorted iteration breaks SC-2 on its own — nondeterministically, so it may pass CI once and fail later. | `workflow_step.rs:329` | Sort by key. Add a proptest that renders the *same* workflow N times and asserts byte-equality (a single-render test cannot catch this). |
| 9 | **`.planning/` artifacts ARE committed.** `.planning/config.json` has `"commit_docs": true`. | Measured | RESEARCH/PLAN/SUMMARY files get committed; `gsd_run query commit` is the mechanism. |
| 10 | **`git mv`, not copy+delete.** A 157 K file re-added as a new path loses `git blame` continuity through the split. | — | `git mv src/server/skills.rs src/server/skills/mod.rs` |
| 11 | **`make quality-gate` is long.** It chains `check-release-coverage`, `fmt-check`, `lint`, `doc-check`, `build`, `test-all`, `test-skills`, `lint-skills`, `pmcp-package-gate`, `audit`, `unused-deps`, `check-todos`, `check-unwraps`, `validate-always`, `purity-check`, `no-crypto-check`, `comply`. | `Makefile:1895-1922` | Run per-leg during the inner loop; run the full gate once before commit. |
| 12 | **`unused-deps` is in the gate.** If the projection adds a dependency (it should not need one), an unused-in-some-feature-combination dep fails the gate. | `Makefile:1917` | The renderer needs nothing beyond `std`, `serde_json` and `sha2` — all already present. |
| 13 | **Phase 125 left WR-03/WR-04/WR-05 open and CR-01 accepted.** WR-03 is a `panic!` in a `Result`-returning build path (`builder.rs:1501`) with user-visible reach; `deferred-items.md:136` names *"the next phase of the v2.7 milestone"* as suggested owner — which is this phase. | `.planning/phases/125-…/deferred-items.md:115-138` | Phase 126 does not inherit an obligation to fix it (D-15 only requires that the projection itself never panics), but the planner should decide explicitly whether to fold the WR-03 fix in or restate the deferral, so it does not silently age out. |
| 14 | **`SequentialWorkflow::instruction()` is a dead public builder method** (CONFLICT 1's second-order note). A projection that renders instructions is the *only* surface that will ever show them. | `grep` (see CONFLICT 1) | Worth a sentence in the projection's module docs: the Context section is where a workflow's `instruction()` text first becomes observable. |

---

## Standard Stack

Zero new dependencies. Everything the phase needs is already in the graph.

### Core

| Library | Version | Purpose | Why standard |
|---|---|---|---|
| `std` (`String`/`fmt`) | — | The renderer | A pure string builder; no templating engine belongs in a 157 K module `[VERIFIED: skills.rs builds every string with push_str]` |
| `sha2` | `0.11` (`Cargo.toml:149`) | D-15 slug fallback hash | Already the module's digest source; reuse `sha256_digest_hex` `[VERIFIED: skills.rs:1801]` |
| `serde_json` | `1.0` + `raw_value`, `preserve_order` (`Cargo.toml:119`) | `DataSource::Constant` rendering; frontmatter `Value` | `preserve_order` is what makes constant rendering deterministic `[VERIFIED: Cargo.toml:119]` |
| `indexmap` | `2.10` + `serde` (`:136`) | Already backs `arguments()` | No new usage needed |
| `tracing` | in-graph | D-10 warn channel, `target: "mcp.skills"` | `[VERIFIED: skills.rs:1011]` |
| `serde_yaml` | `0.9`, optional, pulled by `skills` (`:158`, `:318`) | Frontmatter parse (already wired) | Untouched by this phase |

### Supporting (dev only)

| Library | Version | Purpose | When |
|---|---|---|---|
| `proptest` | `1.7` (`:245`) | SC-2 determinism + slug invariants | The harness this area already uses `[VERIFIED: skills.rs:2381, skills_integration.rs:477]` |
| `libfuzzer-sys` / `arbitrary` | via `fuzz/Cargo.toml` | ALWAYS/FUZZ | `fuzz/Cargo.toml:60` already enables `skills` on the pmcp dep |

### Alternatives Considered

| Instead of | Could use | Tradeoff |
|---|---|---|
| Hand-built `String` | `askama` / `handlebars` / `minijinja` | New dependency on a crate whose output stability is not under our control, in a module whose *entire point* is byte-stability. Rejected. |
| `include_str!` golden | `insta` snapshots | `insta` is a dev-dep (`Cargo.toml:253`) but used **nowhere**; introducing `cargo insta review` into a gate with no story for it is scope. Rejected — but note the dep is already paid for if a later phase wants it. |
| `&HashMap<String, ToolInfo>` for `with_tools` | `impl IntoIterator<Item = ToolInfo>` | The iterator form accepts `Vec<ToolInfo>` straight off a `tools/list` result; the map form forces callers to build a map. Prefer the iterator. |

**Installation:** none — no manifest dependency changes. The only `Cargo.toml` edit is the `[[example]]` stanza (Q9).

## Package Legitimacy Audit

**Not applicable — this phase installs no external packages.** No dependency is added to `Cargo.toml`, `fuzz/Cargo.toml`, or any workspace manifest. Every crate named in `## Standard Stack` is already a resolved dependency of `pmcp` at HEAD (`Cargo.toml:119,136,149,158,245,253`). The single manifest change is a `[[example]]` stanza, which declares no dependency.

---

## Architecture Patterns

### System Architecture Diagram

```
                  ┌──────────────────────────────────────────────────┐
   author code ──▶│ SequentialWorkflow                               │
                  │  name · description · arguments(IndexMap)        │
                  │  steps(&[WorkflowStep]) · instructions(&[msg])   │
                  └───────────────┬──────────────────────────────────┘
                                  │ (public introspection — read-only, no new accessors)
                 ┌────────────────┴─────────────────┐
                 │                                  │
   D-01 ─────────▼──────────                        ▼──────────────── D-04
 as_skill() -> Skill              SkillProjection::new(&wf)
 (infallible, warns via             .with_tools([ToolInfo …])   ── D-07 annotations
  tracing on slug fallback)          .build() -> Result<Skill>
                 │                             │  + Vec<ProjectionWarning>  ── D-10
                 └──────────────┬──────────────┘
                                │  BOTH delegate to ↓
                  ┌─────────────▼────────────────────────────────────┐
                  │ src/server/skills/projection.rs  (D-03, new)     │
                  │  slugify → 1-64 [a-z0-9-] | workflow-{8 hex}     │  ── D-15
                  │  render_frontmatter (name+description only)      │  ── D-13
                  │  render_context   ← instructions, PromptContent  │
                  │  render_inputs    ← ArgumentSpec                 │
                  │  render_procedure ← steps                        │
                  │    render_step: tool · args · binding · guidance │
                  │                  · resources · template_bindings │  ── D-11
                  │                  (SORTED — HashMap!)             │
                  │  render_closing   ← names the prompt, no redirect│  ── D-06
                  │  gate_check       ← guidance × ToolAnnotations   │  ── D-09
                  └─────────────┬────────────────────────────────────┘
                                │ ONE String  (D-05)
                 ┌──────────────┴──────────────────┐
                 │                                 │
                 ▼                                 ▼
   Skill::new(slug, body)              PromptMessage message[0]
                 │                       (WorkflowPromptHandler
                 │                        + TaskWorkflowPromptHandler,
                 │                        opt-in flag)
                 ▼
   Skills::add → into_handler()  ── existing, untouched
     ├─ validate_names        (name == final URI segment) ✓ SC-1 by construction
     ├─ validate_unique_uris  (Err on duplicates)         ✓ D-15
     └─ skill_resource_manifest → {uri, sha256 digest, size}
                 │
                 ▼
   skills/list · skills/get · resources/read  ── wire, Phase 125 ✓ SC-4
```

Data flows one way: workflow → renderer → one `String` → two consumers. The renderer holds no state and performs no I/O, which is what makes SC-2 (byte-equal re-derivation) provable rather than merely observed.

### Recommended Project Structure

```
src/server/skills/
├── mod.rs            # git mv from skills.rs — Skill, SkillReference, Skills,
│                     # SkillEntry, SkillsHandler, validate_names, sha256_digest_hex
└── projection.rs     # NEW — SkillProjection, ProjectionWarning, slugify, render_*
                      #        + #[cfg(test)] mod tests (unit + proptest)

src/server/workflow/sequential.rs   # + #[cfg(feature = "skills")] pub fn as_skill(&self) -> Skill
src/server/workflow/prompt_handler.rs        # + prepend flag + shared projected_prepend()
src/server/workflow/task_prompt_handler.rs   # + same flag honored on its own message path

tests/golden/workflow_skill_projection.md    # NEW — D-14 golden
tests/skills_integration.rs                  # + golden test, byte-identity, D-04 transcripts
tests/skills_routing.rs                      # + SC-4 wire proof, fuzz-registration tripwire
fuzz/fuzz_targets/fuzz_workflow_projection.rs # NEW + [[bin]] stanza in fuzz/Cargo.toml
examples/s56_workflow_skill_projection.rs    # NEW + [[example]] stanza
```

### Pattern 1: Infallible/fallible constructor pair (D-02)

**What:** a `#[must_use]` infallible method that delegates to a `Result`-returning twin.
**When:** whenever the strict form can reject input the lenient form must tolerate.

```rust
// Source: src/server/skills.rs:362-397 (this repo, verbatim shape)
#[must_use]
pub fn with_reference(self, reference: SkillReference) -> Self {
    match self.try_with_reference(reference) {
        Ok(s) => s,
        Err(e) => panic!("Skill::with_reference: {e}"),
    }
}

pub fn try_with_reference(mut self, reference: SkillReference) -> Result<Self> {
    validate_reference_path(&reference.relative_path, &self.references)?;
    self.references.push(reference);
    Ok(self)
}
```

**Deviation D-15 requires:** the existing pair *panics* on the infallible side. `as_skill()` must **warn and fall back**, never panic. Follow the *shape* of the pair, not this body.

### Pattern 2: `*_with_diagnostics` + public logging wrapper (D-10)

**What:** a crate-private function returning `(product, Vec<Diagnostic>)`, wrapped by a public method that emits `tracing::warn!`.
**When:** a build-time finding must be both assertable in tests and visible to operators.

```rust
// Source: src/server/skills.rs:1008-1018 (verbatim)
pub fn entries(&self) -> Result<Vec<SkillEntry>> {
    let (entries, diagnostics) = self.entries_with_diagnostics()?;
    for diagnostic in &diagnostics {
        tracing::warn!(
            target: "mcp.skills",
            uri = %diagnostic.uri(),
            "{}",
            diagnostic.message()
        );
    }
    Ok(entries)
}
```

**Deviation D-10 requires:** `build()` **returns** the warnings to the caller, so `ProjectionWarning` is public where `SkillDiagnostic` (`skills.rs:617`) is deliberately crate-private. Say why in the type's rustdoc.

### Pattern 3: Feature-gated inherent method on an ungated type (D-01)

```rust
// Source: src/server/mod.rs:4683-4687 (verbatim)
#[cfg(feature = "skills")]
#[must_use]
pub fn skill(self, skill: skills::Skill) -> Self {
    self.skills(skills::Skills::new().add(skill))
}
```

### Pattern 4: In-process handler drive (SC-4, D-04 tests)

```rust
// Source: tests/skills_integration.rs:109-127
let handler: Arc<dyn ResourceHandler> = Skills::new().add(skill).into_handler()?;
let main = handler.read("skill://refund-flow/SKILL.md", RequestHandlerExtra::default()).await?;
// …and for the prompt surface (spike 009 main.rs:328-332; CONVENTIONS.md:102-106):
let ph = WorkflowPromptHandler::new(wf, infos, handlers, None::<Arc<dyn ResourceHandler>>);
let result = ph.handle(args, RequestHandlerExtra::default()).await?;
```

`Server::handle_request` is private — external code cannot drive a `pmcp::Server` in-process by JSON-RPC (`CONVENTIONS.md:101-106`). Use the trait method directly, or the loopback fixtures already in `tests/skills_routing.rs`.

### Anti-Patterns to Avoid

- **Debug-formatting protocol content into a digested body.** Spike 009's only real bug (`main.rs:119`, `{other:?}`); with `#[non_exhaustive]` enums a future variant silently changes the digest.
- **Iterating a `HashMap` into rendered output.** Nondeterministic; may pass CI once (`template_bindings`, `workflow_step.rs:329`).
- **Adding a new `tests/*.rs` for skills work.** Invisible to all four `make test-skills` selectors (`Makefile:1021-1086`).
- **Trusting `make test-fuzz` / `make validate-always` as fuzz evidence.** Exits 0 having run nothing on stable — measured, Q7.
- **Using `cargo nextest -E 'test(/…/)'`** — silently selects zero tests; the Makefile records this at `:951`.
- **Adding `skills` to `full`/`full-v2`** — fails `tests/v1_severability_tripwire.rs::full_and_full_v2_differ_by_exactly_v1_compat` (`:143`).
- **Struct-literal syntax on `#[non_exhaustive]` types** (`ToolAnnotations`, `ToolInfo`, `PromptContent`, `DataSource`, `GetPromptResult`).
- **Re-implementing the SHA-256 hex encoder.** `sha2` 0.11 has no `LowerHex`; `skills.rs:1801` already solves it.

---

## Don't Hand-Roll

| Problem | Don't build | Use instead | Why |
|---|---|---|---|
| `name` == final URI segment enforcement | A projection-side identity check | `validate_names` (`skills.rs:1444`), reached automatically via `into_handler`/`entries` | Already runs at both choke points; a second check can only disagree |
| Duplicate skill/reference URI detection | A collision map in the projection | `Skills::into_handler` → `build_handler` (`skills.rs:1137-1190`) | Catches SKILL.md↔SKILL.md, ref↔ref, **and** ref↔SKILL.md cross-collisions; `entries` runs the twin `validate_unique_uris` (`:1385`) |
| `{uri, digest, size}` manifest | Manual digest walk | `skill_resource_manifest` (`skills.rs:1487`) | Digests the exact `&str` `SkillsHandler::read` returns — cannot drift by construction (`:1472-1486`) |
| SHA-256 hex | `format!("{:x}", …)` | `sha256_digest_hex` (`skills.rs:1801`) | No `LowerHex` in sha2 0.11 / digest 0.11.2 / crypto-common 0.2.2 |
| Verbatim frontmatter → JSON | A YAML re-parse in projection | `build_artifact_inner` / `parse_frontmatter_value` (`skills.rs:1240`, `:1677`) | One parse per build, by design; a second parse is a drift source |
| Dual-surface text | A second body variant for the prompt | `Skill::as_prompt_text()` (`skills.rs:451`) | D-05's whole point |
| Capability declaration | Manual `ServerCapabilities` edit | `set_skills_capabilities` (`skills.rs:206`) | Called by `.skill`/`.skills`; a projected skill inherits it |
| Fuzz-target scheduling proof | Hoping `make test-fuzz` runs it | Copy `tests/skills_routing.rs:1431` `fuzz_skill_entry_is_registered_and_scheduled` | The only thing that actually fails when a target is unregistered |

**Key insight:** the projection's *entire* conformance story is inherited. Every custom check the projection adds is a second opinion that can only disagree with the choke point Phase 125 built. The only things Phase 126 genuinely owns are: slugification, deterministic rendering, and the SC-6 warning.

---

## Common Pitfalls

### Pitfall 1: The determinism test that cannot fail

**What goes wrong:** `assert_eq!(wf.as_skill().body(), wf.as_skill().body())` passes even with an unsorted `HashMap` render, because two calls in one process often hash-iterate identically within a single `RandomState`… and then differ across processes or across a rehash.
**Why:** Rust's `HashMap` randomizes per-`RandomState`, which is per-map-construction, and both calls read the *same* map instance.
**How to avoid:** the determinism test must (a) render N ≥ 100 times, **and** (b) construct the workflow fresh each time, **and** (c) be complemented by a *golden file* comparison — which is exactly why D-14 requires both halves.
**Warning signs:** a determinism test that never touched `template_bindings`.

### Pitfall 2: A green test leg that ran zero tests

**What goes wrong:** a skills test placed under `--features "full"` compiles to nothing (the module is `#[cfg]`'d out) and the run reports `0 passed; 0 failed` with exit 0.
**Why:** `skills` is in neither `default` nor `full` (`Cargo.toml:279-295`).
**How to avoid:** `make test-skills` already guards every selector with a zero-count check (`Makefile:1029-1032` etc.). Any new verify command in a plan must either be one of those four selectors or carry its own count assertion.
**Warning signs:** a `cargo test --features "full"` command in a plan that names a skills test.

### Pitfall 3: A doctest that never runs

**What goes wrong:** the `as_skill()` doctest lives at `pmcp::server::workflow::sequential`, so `make test-skills` selector 2 (`--doc skills`) filters it out; and `test-doc` inside `test-all` pins `--features "full"`, under which the method does not exist.
**Why:** the substring filter and the feature set disagree about where "skills code" lives.
**How to avoid:** run `cargo test -p pmcp --features "skills,full" --doc sequential -- --test-threads=1` explicitly, and put that command in the plan's verify block.
**Warning signs:** "doctest added" with no command that could have executed it.

### Pitfall 4: Byte-equality that silently means "both empty"

**What goes wrong:** `assert_eq!(skill.as_prompt_text(), skill.body())` on a skill whose body is `""`.
**Why:** vacuous equality.
**How to avoid:** anti-vacuity first — assert `body.len() > 200`, `body.starts_with("---\nname: ")`, `body.ends_with('\n')`, and `skill.references().count() == 0` before the equality. This is the house style (`tests/skills_routing.rs:1459-1464` does exactly this for a manifest scan).
**Warning signs:** an equality assertion with no positive assertion beside it.

### Pitfall 5: `String::truncate` panic on a 64-byte cut

**What goes wrong:** truncating a UTF-8 string at byte 64 mid-codepoint panics.
**Why:** `String::truncate` asserts a char boundary.
**How to avoid:** truncate only *after* the ASCII-reducing map (step 2 of the Q3 algorithm) — the string is pure ASCII at that point, so byte index == char index. Comment the invariant; it is not visually obvious and a later "optimization" that reorders the steps reintroduces the panic. D-15 says **nothing panics**.
**Warning signs:** a `truncate` call before the `is_ascii_alphanumeric` map.

### Pitfall 6: Prepending in only one of the two prompt handlers

**What goes wrong:** `has_task_support(true)` workflows get a different transcript from plain ones.
**Why:** `TaskWorkflowPromptHandler::handle` re-builds its own message list at `task_prompt_handler.rs:691-692` instead of always delegating.
**How to avoid:** one shared `pub(crate)` producer on `WorkflowPromptHandler`; both call sites use it; a test constructs both handler kinds over the same workflow and asserts message [0] is byte-identical.
**Warning signs:** a plan that touches `prompt_handler.rs` and not `task_prompt_handler.rs`.

---

## Code Examples

Verified patterns, all from this repository.

### Slug + fallback (assembling Q3's pieces against live APIs)

```rust
// slugify: spike 009 main.rs:68-77 + the 1-64 bound from agentskills.io/specification#name-field
fn slugify(name: &str) -> Option<String> {
    let mut s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|seg| !seg.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if s.is_empty() {
        return None;
    }
    if s.len() > 64 {
        // Safe: after the map above every byte is ASCII, so byte index == char boundary.
        s.truncate(64);
        while s.ends_with('-') {
            s.pop();
        }
        if s.is_empty() {
            return None;
        }
    }
    Some(s)
}

// D-15 fallback — reuses super::sha256_digest_hex (src/server/skills.rs:1801)
fn fallback_slug(original: &str) -> String {
    let digest = super::sha256_digest_hex(original.as_bytes()); // "sha256:" + 64 hex
    let hex = &digest["sha256:".len()..];
    format!("workflow-{}", &hex[..8])
}
```

### Deterministic template-binding render (SC-2)

```rust
// WorkflowStep::template_bindings() -> &HashMap<String, DataSource>  (workflow_step.rs:329)
let mut keys: Vec<&String> = step.template_bindings().keys().collect();
keys.sort_unstable(); // byte order — locale-independent, total (keys are unique)
for k in keys {
    let ds = &step.template_bindings()[k];
    out.push_str(&format!("- `{{{k}}}`: {}\n", render_data_source(ds)));
}
```

### `DataSource` render with a stable catch-all

```rust
// DataSource is #[non_exhaustive] (data_source.rs:9) — the `_` arm is REQUIRED
// and must emit a CONSTANT, never `{:?}`.
fn render_data_source(ds: &DataSource) -> String {
    match ds {
        DataSource::PromptArg(a) => format!("use the `{}` argument you were given", a.as_str()),
        DataSource::StepOutput { step, field: None } =>
            format!("use the entire saved `{}` result", step.as_str()),
        DataSource::StepOutput { step, field: Some(f) } =>
            format!("use field `{f}` of the saved `{}` result", step.as_str()),
        DataSource::Constant(v) =>
            format!("use the constant value `{}`", serde_json::to_string(v)
                .unwrap_or_else(|_| "null".to_string())),
        _ => "(unsupported data source)".to_string(),
    }
}
```

### `tracing::warn!` on the `mcp.skills` target (D-10)

```rust
// Source: src/server/skills.rs:1011-1016 (verbatim)
tracing::warn!(
    target: "mcp.skills",
    uri = %diagnostic.uri(),
    "{}",
    diagnostic.message()
);
```

### Self-gated doctest on a `skills`-gated public item

```rust
// Source shape: src/server/mod.rs:4670-4681, src/server/skills.rs:528
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "skills")] {
/// use pmcp::server::workflow::SequentialWorkflow;
///
/// let wf = SequentialWorkflow::new("refund_flow", "Process a customer refund");
/// let skill = wf.as_skill();
/// assert_eq!(skill.name(), "refund-flow");
/// assert_eq!(skill.as_prompt_text(), skill.body());
/// # }
/// ```
```

---

## State of the Art

| Old approach | Current approach | When changed | Impact |
|---|---|---|---|
| Skill discovery via a synthesized `skill://index.json` resource | `skills/list` + `skills/get` methods with complete entries | Phase 125 (SDK); SEP-2640 draft | The projected skill needs no discovery work — it appears in `skills/list` automatically |
| `skills/list` entry as a summary to be completed by a follow-up call | Entry is a **complete manifest**: verbatim `frontmatter` + full `resources` with per-file digests | SEP-2640 current draft `[CITED: seps/2640-skills-extension.md:568]` | The digest a consumer pins is published in the listing — D-14's "re-pin on render change" is a real cost, not a theoretical one |
| `sha2` 0.10 with `format!("{:x}", digest)` | `sha2` 0.11, manual nibble-table hex | Phase 125 D-05 | The spike-era snippet does not compile |
| Frontmatter extensions as ad-hoc top-level keys | `metadata:` map (string→string), with `io.modelcontextprotocol/`-prefixed keys reserved | agentskills spec + SEP-2640 `:243` | Unblocks the deferred provenance-key idea (see the note under Deferred Ideas) |

**Deprecated / outdated:**
- **Spike 009's `project_skill()` as a template** — its `{other:?}` fallback (`main.rs:119`), missing length bound, and unsorted-`HashMap` blindness are all defects the shipped renderer must not inherit. Read it for the *mapping*, not for the code.
- **The "Server-accelerated alternative" section wording** — superseded by D-06's user-education framing.
- **`skill://index.json`** — retired in Phase 125 plan 04; every remaining occurrence in the tree is a test asserting its absence.

---

## Assumptions Log

| # | Claim | Section | Risk if wrong |
|---|---|---|---|
| A1 | The right resolution of CONFLICT 1 is option (b) — prepend and suppress `create_assistant_plan` | Conflicts / Q4 | Wrong transcript shape ships behind a flag callers then depend on (D-04 marks this "costly" reversibility). **This is a user decision, not an assumption to act on.** |
| A2 | `s56` is the correct free example number | Conflicts / Q9 | A collision would surface immediately at `cargo build --examples`; low risk, cheap to fix |
| A3 | `ProjectionWarning` should be public (D-10 says `build()` returns warnings) while the module's `SkillDiagnostic` precedent is crate-private | Q6 | Adds public API surface that `cargo public-api` will see; if the planner prefers, the warnings can be an opaque public struct with accessors rather than a public enum |
| A4 | `impl IntoIterator<Item = crate::types::ToolInfo>` is the best `with_tools` signature | Q6 | An ergonomics call, not a correctness one; reversible while unreleased |
| A5 | The `Image` variant should render its `mime_type` and drop `data` | Q2 | If a real workflow puts an instruction image in `instructions()`, the projected skill loses it. No known such workflow exists in-repo (`instructions()` has one non-test consumer, an example that only prints) |
| A6 | Compact (`to_string`) rather than pretty JSON for `DataSource::Constant` | Q2 | Readability preference; changes the golden bytes if revisited later |
| A7 | `tests/golden/` is an acceptable new directory | Q7 | `tests/` currently holds only `common/` and `integration/` subdirs; a `tests/golden/*.md` file is not a test target and will not be compiled as one — but confirm no `[[test]]` autodiscovery surprise before relying on it |
| A8 | `tests/workflow_prompt_e2e_test.rs` may not admit the `skills` feature | Q10 | Its `#![cfg]` header was not read this session; the recommendation already routes D-04 tests elsewhere, so the risk is contained |

---

## Open Questions

1. **What exactly does the D-04 flag suppress?** (CONFLICT 1)
   - **What we know:** `instructions()` reaches no production path; messages [0] and [1] are `create_user_intent` and `create_assistant_plan`; the projected body duplicates the content of both.
   - **What's unclear:** whether D-04's intent, restated against reality, means prepend-only, prepend+suppress-plan, or prepend+suppress-both.
   - **Recommendation:** raise with the user before the D-04 plan is written. Default to prepend + suppress `create_assistant_plan`, but do not treat that as settled.

2. **Does `Skills::from_workflow(&wf)` also ship?** (Claude's discretion)
   - **What we know:** `Skills` has `new()` (`:849`), `add()` (`:856`), `merge()` (`:865`) — a `from_workflow` would be a natural `Skills::new().add(wf.as_skill())`.
   - **Recommendation:** **no.** It is a one-line composition of two shipped methods and adds a second discoverable name for one concept — the exact drift surface D-01 is trying to avoid. If discoverability is the worry, mention `as_skill()` in `Skills`'s module rustdoc instead.

3. **Does `SequentialWorkflow::validate()` learn the D-09 check?** (Claude's discretion, declined as an option)
   - **What we know:** `validate()` (`sequential.rs:239`) is ungated and returns `Result<(), WorkflowError>`; `ServerCoreBuilder::prompt_workflow` calls it at `:1216-1218` and converts a failure into `Error::validation`. Adding a gate check there would make every existing side-effecting-step-with-guidance workflow **fail to register** — a hard behavioural break.
   - **Recommendation:** **no**, and say why in the projection module docs. The seam is adjacent but the semantics are wrong: D-09's finding is advisory, `validate()` is fatal.

4. **`ProjectionWarning`: enum or struct?** (Claude's discretion)
   - **What we know:** D-08 requires two distinct diagnostics (gate warning vs. "unverifiable"); D-15 adds a third (slug fallback). `SkillDiagnostic` (`skills.rs:617`) is an enum with a `uri()` and a `message()` accessor pair, which is what its tests assert on.
   - **Recommendation:** a **`#[non_exhaustive]` public enum** with `step_name()`, `tool_name() -> Option<&str>` and `message()` accessors, mirroring `SkillDiagnostic`'s accessor shape so tests read the same way. `#[non_exhaustive]` keeps future variants additive.

5. **Does `arg_type` reach the Inputs section?** (Claude's discretion)
   - **What we know:** `ArgumentSpec.arg_type: Option<PromptArgumentType>` is public (`sequential.rs:49`) and free to read.
   - **Recommendation:** render it when `Some` (`` - `order_id` (required, string): The order to refund ``) and omit the parenthetical type when `None`. It is genuine manual-runner information and costs nothing — but it is a golden-bytes decision, so make it once and pin it.

---

## Environment Availability

| Dependency | Required by | Available | Version | Fallback |
|---|---|---|---|---|
| Rust stable toolchain | everything | ✓ | `rustc 1.98.0 (88d9e12ae 2026-08-18)`, `cargo 1.98.0` | — |
| Rust nightly toolchain | real fuzz runs | ✓ | `nightly-aarch64-apple-darwin` installed | — |
| `cargo-fuzz` | ALWAYS/FUZZ | ✓ (binary present) | `/Users/guy/.cargo/bin/cargo-fuzz` | **`cargo fuzz run` FAILS on stable** (`-Zsanitizer=address` is nightly-only); use `cargo +nightly fuzz run` |
| `pmat` | CI complexity gate + `make comply` | ✓ | `pmat 3.15.0` (matches the CI pin) | — |
| `make` / GNU-compatible shell | every gate leg | ✓ | Makefile runs | — |
| `git` | `git mv` for the D-03 split | ✓ | repo is a git worktree on `main` | — |
| Network (crates.io API) | `make release-sweep` only | not needed | — | Not chained into `quality-gate` (`Makefile:1138-1146`) |
| `cargo-deny` 0.18.3 | `make purity-check` (in `quality-gate`) | UNVERIFIED | — | Would verify with `cargo deny --version`; if absent, `purity-check` fails for a reason unrelated to this phase |
| `cargo-audit`, `cargo-udeps`/`unused-deps` tooling | `make audit`, `make unused-deps` | UNVERIFIED | — | Same class as above — verify at Wave 0 by running `make quality-gate` once on an unmodified tree |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** `cargo fuzz run` on stable → `cargo +nightly fuzz run`.

---

## Validation Architecture

### Test Framework

| Property | Value |
|---|---|
| Framework | Rust built-in `#[test]` / `#[tokio::test]` + `proptest 1.7` (`Cargo.toml:245`) |
| Config file | none — `Cargo.toml [dev-dependencies]` + `Makefile` targets |
| Quick run command | `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --lib skills::projection -- --test-threads=1` |
| Full suite command | `make test-skills` (four guarded selectors, `Makefile:1021-1086`) |
| Phase gate | `RUSTFLAGS="" make quality-gate` **plus** `pmat quality-gate --fail-on-violation --checks complexity` (the latter is CI-only and NOT in `make quality-gate` — CLAUDE.md D-07) |
| Estimated runtime | quick: ~2 s after warm build; `make test-skills`: ~1–2 min; full gate: tens of minutes |

> **Never use `make test-unit`, `make test-integration`, `make test-property` or `make test` as a skills verify** — all pin `--features "full"`, which excludes `skills`; they report success having run zero tests from this module (`Makefile:230`, `:781-784`, and the in-Makefile explanation at `:1905-1909`).
> **Never use `cargo nextest -E 'test(/…/)'`** — silently selects zero tests (`Makefile:951`). Use `binary(<name>)` if nextest is unavoidable.
> **Always prefix `RUSTFLAGS=""`** locally — CI exports it, local shells do not, and `make test-skills` sets `RUSTFLAGS=` per leg.

### Phase Requirements → Test Map

| SC / D | Behavior | Test type | Automated command | File exists? |
|---|---|---|---|---|
| **SC-1** | `refund_flow` → skill `refund-flow`; final URI segment == frontmatter `name`; `into_handler()`/`entries()` accept it | unit + integration | `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --lib skills::projection -- --test-threads=1` | ❌ Wave 0 (`src/server/skills/projection.rs`) |
| **SC-1** (slug legality) | slug ∈ `[a-z0-9-]`, len 1..=64, no leading/trailing/consecutive `-`; fallback `workflow-{8hex}` for names with nothing legal | **property** (proptest over arbitrary `String` workflow names) | same as above | ❌ Wave 0 |
| **SC-2** | Re-deriving a **freshly constructed** identical workflow N≥100 times yields byte-equal bodies (catches the `template_bindings` `HashMap`) | **property** | same as above | ❌ Wave 0 |
| **SC-2** (other half) | Body matches the committed golden byte-for-byte | golden | `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --test skills_integration -- --test-threads=1` | ❌ Wave 0 (`tests/golden/workflow_skill_projection.md`) |
| **SC-3** | Every workflow fact appears: name, description, each argument name+description+required, each step's tool name, each argument binding, each `with_guidance` line, each resource, each template binding — asserted **string-by-string**, never by length or a single `contains` | unit | `--lib skills::projection` | ❌ Wave 0 |
| **D-11 (exclusions)** | `is_retryable` / `has_task_support` produce **no** text — assert the body does NOT contain the marker strings, over a workflow that sets both to non-default | unit | `--lib skills::projection` | ❌ Wave 0 |
| **SC-4** | Projected skill registers; `skill://{name}/SKILL.md` reads back byte-identical through the real `ResourceHandler` | integration (in-process) | `--test skills_integration` | ✓ `tests/skills_integration.rs` (extend) |
| **SC-4** (wire) | `skills/list` over a real loopback `StreamableHttpServer` returns the projected skill's entry with verbatim `{name, description}` frontmatter and a complete `{uri, digest: "sha256:"+64hex, size}` manifest | integration (wire) | `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --test skills_routing -- --test-threads=1` | ✓ `tests/skills_routing.rs` (extend) |
| **SC-5** | `skill.as_prompt_text() == skill.body()`, with anti-vacuity: `references().count() == 0`, `body().ends_with('\n')`, `body().len() > 200` | unit | `--lib skills::projection` | ❌ Wave 0 |
| **SC-5** (surface equivalence) | The set of tool names in the rendered Procedure **equals** the set `wf.steps().filter_map(WorkflowStep::tool)` names — set equality, not one-way containment | unit | `--lib skills::projection` | ❌ Wave 0 |
| **SC-6 / D-08 / D-09** | `read_only_hint == Some(false)` or `destructive_hint == Some(true)` on a guidance-bearing step ⇒ exactly one gate warning naming that step; annotations absent ⇒ exactly one *distinct* "unverifiable" note; `with_tools` never called ⇒ zero diagnostics | unit (assert on returned `Vec<ProjectionWarning>`, no `tracing` subscriber) | `--lib skills::projection` | ❌ Wave 0 |
| **D-04** | With the flag: message [0] text == `skill.body()`. Without: transcript byte-identical to today's. Both `WorkflowPromptHandler` and `TaskWorkflowPromptHandler` produce the same message [0]. | integration (in-process `PromptHandler::handle`) | `--test skills_integration` | ✓ `tests/skills_integration.rs` (extend) — **not** `prompt_handler.rs`'s `mod tests`, which no gate leg reaches |
| **D-14** | The golden-mismatch failure message names the file **and** tells the operator to add a CHANGELOG entry because pinning consumers must re-pin | golden (message assertion) | `--test skills_integration` | ❌ Wave 0 |
| **ALWAYS/FUZZ** | `as_skill()` does not panic on arbitrary workflow names / descriptions / guidance / constants | fuzz | **registration:** `--test skills_routing` (copy `fuzz_skill_entry_is_registered_and_scheduled`, `:1431`). **execution:** `cd fuzz && cargo +nightly fuzz run fuzz_workflow_projection -- -runs=200000 -max_total_time=30` | ❌ Wave 0 |
| **ALWAYS/EXAMPLE** | s56 runs and asserts (not merely prints) | example | `cargo run --example s56_workflow_skill_projection --features skills,full` | ❌ Wave 0 |
| **ALWAYS/DOCTEST** | `as_skill()` and `SkillProjection::{new,with_tools,build}` doctests execute | doctest | `cargo test -p pmcp --features "skills,full" --doc sequential -- --test-threads=1` **and** `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --doc skills -- --test-threads=1` | ❌ Wave 0 |
| **CLAUDE.md / PMAT** | No `src/` function exceeds cog 25 | static | `pmat quality-gate --fail-on-violation --checks complexity` (baseline today: `Total violations: 0`) | ✓ tool installed |
| **Feature discipline** | `full` / `full-v2` untouched | static | `cargo test -p pmcp --features "full" --test v1_severability_tripwire -- --test-threads=1` | ✓ `tests/v1_severability_tripwire.rs` |

### Sampling Rate

- **Per task commit:** `RUSTFLAGS="" cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --lib skills::projection -- --test-threads=1` (~2 s warm)
- **Per wave merge:** `RUSTFLAGS="" make test-skills` **and** `RUSTFLAGS="" make lint-skills` **and** `pmat quality-gate --fail-on-violation --checks complexity`
- **Phase gate:** `RUSTFLAGS="" make quality-gate` green, plus the two commands `make quality-gate` cannot run — `pmat quality-gate --fail-on-violation --checks complexity` and `cargo test -p pmcp --features "skills,full" --doc sequential -- --test-threads=1` — plus one manual `cargo run --example s56_workflow_skill_projection --features skills,full`
- **Max feedback latency:** ~5 s (quick command, warm build)

`--test-threads=1` is mandatory (CLAUDE.md) and this workspace has recorded parallel-test races.

### Wave 0 Gaps

- [ ] `git mv src/server/skills.rs src/server/skills/mod.rs` + `pub mod projection;` — the split must land before any renderer code, or every subsequent file moves
- [ ] `src/server/skills/projection.rs` with `#[cfg(test)] mod tests` — the home for all unit + property tests (path must contain `skills` for selector 1)
- [ ] `tests/golden/workflow_skill_projection.md` — the D-14 golden; confirm no `[[test]]` autodiscovery treats `tests/golden/` as a target
- [ ] `fuzz/fuzz_targets/fuzz_workflow_projection.rs` + `[[bin]]` stanza in `fuzz/Cargo.toml` (features already enable `skills` at `:60`) + the registration tripwire copied from `tests/skills_routing.rs:1431` + a CI fuzz-matrix row in `.github/workflows/fuzz.yml`
- [ ] `examples/s56_workflow_skill_projection.rs` + `[[example]]` stanza after `c10_client_skills` in `Cargo.toml`
- [ ] **Resolve CONFLICT 1 with the user** before the D-04 plan is written
- [ ] Read `tests/workflow_prompt_e2e_test.rs`'s `#![cfg]` header (5.2 K) to confirm whether it can host D-04 tests (recommendation is to use `skills_integration.rs` regardless)
- [ ] Check `../provable-contracts/contracts/pmcp/` for an existing skills contract (CLAUDE.md contract-first; `make comply` is in the gate)
- [ ] Run `RUSTFLAGS="" make quality-gate` once on the unmodified tree to establish that `purity-check` / `audit` / `unused-deps` tooling is present locally

---

## Security Domain

Phase 126 adds no network surface, no parsing of untrusted bytes at a new boundary, and no authentication or authorization path. The projection is a pure function over server-author-supplied data. Applicable controls:

### Applicable ASVS Categories

| ASVS category | Applies | Standard control |
|---|---|---|
| V2 Authentication | no | No auth surface touched |
| V3 Session Management | no | — |
| V4 Access Control | no | The projected skill is served through the existing `SkillsHandler`, unchanged |
| V5 Input Validation | **yes** | Slug normalization is the validation boundary: the algorithm is total (`Option`), enforces the agentskills character set and 1-64 bound, and never panics (D-15). The fuzz target is the ASVS evidence. |
| V6 Cryptography | **yes (reuse only)** | SHA-256 via the existing `sha256_digest_hex` (`skills.rs:1801`). **Never hand-roll.** Note SEP-2640 is explicit that digests are **not** an integrity boundary — `SkillResourceRef`'s rustdoc (`skills.rs:520-528`) already says so; the projection inherits that framing and must not claim more. |
| V7 Error handling & logging | **yes** | `tracing::warn!` on `target: "mcp.skills"` (D-10). The warning message embeds the workflow name and step name — author-supplied strings. Phase 125 already hit this class: `src/server/core.rs:2563` gained control-character neutralization for log injection (WR-04). **Apply the same neutralization to any author string interpolated into a warning.** |
| V12 Files & resources | **yes** | The projected body becomes a served resource. `Skill` construction routes through `Skills::into_handler`'s duplicate-URI and name-identity checks, and the SEP-2640 limits guard (`MAX_SKILL_RESOURCES = 512`, `skills.rs:778`; `MAX_SKILL_TOTAL_BYTES = 16 MiB`) still applies. |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard mitigation |
|---|---|---|
| Unbounded body from a pathological workflow (e.g. a huge `DataSource::Constant`, or an `Image` variant's base64 `data`) crossing the SEP-2640 16 MiB limit | Denial of Service | Never render `PromptContent::Image`'s `data` (Q2); the existing `exceeds_skill_limits` (`skills.rs:807`) warns but does **not** reject — document that a caller producing a >16 MiB projection gets a warning, not an error |
| Log injection via a workflow name / step name / guidance containing newlines or ANSI control characters | Tampering (of logs) | Neutralize control characters before interpolating into `tracing::warn!` — the Phase 125 precedent at `src/server/core.rs:2563` |
| Prompt injection *through* the projected body when it is prepended as message [0] (D-04) | Elevation of Privilege | The body is composed from server-author-supplied workflow definitions, i.e. the same trust level as the rest of the prompt handler's output — **not** a new trust boundary. But SEP-2640 § Security Implications treats **skill content as untrusted model input** in the *host* direction, and the tri-surface reference requires origin visibility. Do not weaken that: the prepended body should be indistinguishable from the skill body (D-05), so a host applying skill-content trust rules sees the same bytes either way. |
| A projected skill silently shadowing a hand-authored one with the same slug | Spoofing | Already fatal: `Skills::into_handler` returns `Err` on duplicate `skill://{name}/SKILL.md` (`skills.rs:1189`). SEP-2640 `:235` additionally makes name collisions the **host's** problem to disambiguate; the SDK's job is to refuse to serve two skills at one URI, which it does. |
| A panic in `as_skill()` reachable from a public infallible API | Denial of Service | D-15's "nothing panics"; the fuzz target is the proof; the `String::truncate` ordering invariant (Pitfall 5) is the specific hazard |

---

## Sources

### Primary (HIGH confidence — read from the live tree this session)

- `src/server/skills.rs` — `Skill` (`:297`), `with_reference`/`try_with_reference` (`:362`/`:393`), `name`/`body`/`resolved_description` (`:400`/`:405`/`:417`), `resolved_path`/`skill_md_uri`/`reference_uri` (`:421`/`:425`/`:429`), **`as_prompt_text` (`:451`)**, `SkillEntry` accessors (`:583-604`), `SkillDiagnostic` (`:617`), `exceeds_skill_limits` (`:807`), `MAX_SKILL_RESOURCES` (`:778`), `Skills::{new,add,merge}` (`:849`/`:856`/`:865`), `entries` + `tracing::warn!` (`:1008-1018`), `entries_with_diagnostics` (`:1045`), `into_handler` (`:1118`), `build_handler` + dup-URI `Err` (`:1137`/`:1189`), `validate_unique_uris` (`:1385`), `validate_names` (`:1444`), `skill_resource_manifest` (`:1487`), `sha256_digest_hex` (`:1801`), proptest strategies (`:2337-2410`)
- `src/server/workflow/sequential.rs` — `SequentialWorkflow` (`:16-35`), `ArgumentSpec` (`:40-50`), `instruction` (`:168`), accessors (`:200`,`:205`,`:210`,`:215`,`:220`,`:225`), `validate` (`:239`)
- `src/server/workflow/workflow_step.rs` — accessors (`:294`,`:299`,`:304`,`:309`,`:314`,`:319`,`:324`,`:329`,`:359`)
- `src/server/workflow/data_source.rs` — `DataSource` (`:9-29`)
- `src/server/workflow/prompt_content.rs` — `PromptContent` (`:13-37`), `InternalPromptMessage` (`:41-46`)
- `src/server/workflow/conversion.rs` — `ToolInfo` (`:17-24`), `PromptContent::to_protocol` (`:49-107`), `InternalPromptMessage::to_protocol` (`:116`)
- `src/server/workflow/prompt_handler.rs` — struct (`:92-103`), `new` (`:129`), `with_middleware_executor` (`:155`), `create_user_intent` (`:406`), `create_assistant_plan` (`:429`), `handle` (`:750-957`) with pushes at `:771`,`:774`,`:805`,`:833`,`:887`,`:893`
- `src/server/workflow/task_prompt_handler.rs` — `TaskWorkflowPromptHandler` (`:201`), `new` (`:227`), `impl PromptHandler` (`:632`), `handle` (`:643`), delegation (`:677`), independent message path (`:691-692`)
- `src/server/mod.rs` — workflow gate (`:164-165`), skills gate + re-export (`:194-201`), `ToolHandler::metadata` (`:366`), skills-gated builder methods (`:4683`,`:4704`,`:4726`,`:4745`)
- `src/server/builder.rs` — `tool_infos` (`:73`), `prompt_workflow` (`:1208-1290`) incl. the annotation-dropping conversion (`:1223-1232`), `finalize_skills_resources` (`:1487`) with the WR-03 `panic!` (`:1501`)
- `src/types/tools.rs` — `ToolAnnotations` (`:20-53`), `ToolInfo` (`:199-230`)
- `src/types/content.rs` — `Role` (`:807-822`)
- `src/lib.rs` — crate lints (`:24-38`); **zero `skills` references**
- `Cargo.toml` — `serde_json` (`:119`), `indexmap` (`:136`), `sha2` (`:149`), `serde_yaml` (`:158`), `proptest` (`:245`), `insta` (`:253`), features (`:278-356`), example stanzas (`:700-717`)
- `Makefile` — `lint` (`:157-…`), `test-property` (`:780-784`), `test-fuzz` (`:786-797`), `test-examples` (`:864`), `SKILLS_FEATURES` (`:954`), `LINT_SKILLS_FEATURES` (`:980`), `lint-skills` (`:982-1019`), `test-skills` (`:1021-1086`), `doc-check` (`:1504-1508`), `quality-gate` (`:1895-1922`), `check-todos` (`:1996-1999`)
- `tests/skills_integration.rs` (`:31` cfg header, `:109-127` in-process handler, `:477`/`:511` proptests, `:504` dual-surface equality)
- `tests/skills_routing.rs` (`:1-10` wire framing, `:1431-1491` fuzz-registration tripwire)
- `tests/v1_severability_tripwire.rs` (`:29`,`:49`,`:66-67`,`:95-98`,`:143`,`:166`)
- `fuzz/Cargo.toml` (`:60` skills feature, `:333-338` bin stanza); `fuzz/fuzz_targets/fuzz_skill_entry.rs` (`:1-30`)
- `scripts/run-example-builds.sh` (`:150`)
- `.github/workflows/ci.yml` (`:113`, `:201-286`)
- `examples/s44_server_skills.rs` (`:1-25`)
- Executed commands: `pmat quality-gate --fail-on-violation --checks complexity` (PASSED, 0 violations); `pmat analyze complexity --max-cognitive 25` (21 violations, all in `tests/`); `cargo fuzz run fuzz_skill_entry -- -runs=1` (**FAILED**, nightly-only `-Z` — output pasted in Q7); `cargo test -p pmcp --all-features --lib skills -- --test-threads=1` (99 passed); `cargo test --all-features --test skills_routing -- --test-threads=1` (20 passed); `cargo test -p pmcp --features "skills,streamable-http,http-client,testing" --test skills_routing` (20 passed); `cargo check -p pmcp --features "skills,streamable-http,http-client,testing" --lib` (clean); module-layout enumeration over `src/`; `rustup toolchain list`; `rustc --version`

### Secondary (MEDIUM confidence — official specs fetched this session)

- `[CITED: https://agentskills.io/specification]` — the `name` field rules (1-64 chars, `a-z0-9-`, no leading/trailing hyphen, no `--`, must match parent directory), the `description` rules (1-1024, non-empty), and the `metadata` extension field
- `[CITED: https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/sep/skills-extension/seps/2640-skills-extension.md]` — fetched 2026-09-02; PR head last pushed `2026-08-29T18:46:46Z` (via `gh pr view 2640`). Lines `:19`, `:41`, `:43`, `:63`, `:68`, `:94`, `:218`, `:235`, `:241`, `:243`, `:269`, `:351`, `:568`, `:583-585`
- `.claude/skills/spike-findings-rust-mcp-sdk/references/skills-positioning-tri-surface.md` — the MANIFEST requirement bullets; naming constraint at `:151-155`
- `.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md` — gap table (`:38`), name-identity framing (`:85-86`)
- `.planning/spikes/009-workflow-skill-projection/README.md` + `src/main.rs` (`:68-77` slugify, `:79-92` render_data_source, `:101-166` project_skill, `:321-375` step 4)
- `.planning/spikes/CONVENTIONS.md` (`:71-109`, `:146-195`)
- `.planning/phases/125-…/125-VERIFICATION.md`, `125-VALIDATION.md`, `deferred-items.md` (`:115-138`)
- `.planning/ROADMAP.md` (`:41-48`, `:107-124`), `.planning/STATE.md`, `.planning/config.json`

### Tertiary (LOW confidence)

- None. No claim in this document rests on an unverified web search or on training knowledge alone. Every `[ASSUMED]`-class item is enumerated in `## Assumptions Log`.

---

## Metadata

**Confidence breakdown:**

| Area | Level | Reason |
|---|---|---|
| Standard stack | HIGH | Zero new dependencies; every version read from `Cargo.toml` this session |
| Module split (Q1) | HIGH | Layout convention enumerated by script; `lib.rs` confirmed skills-free by grep; no functional path references |
| Render surfaces (Q2) | HIGH | Every accessor line-cited and its return type read from source |
| Slugification (Q3) | HIGH | SDK rule quoted verbatim; agentskills rule fetched from the authoritative spec page; SEP-2640 delegation quoted from the PR head branch |
| D-04 seam (Q4) | HIGH (finding) / **BLOCKED** (decision) | The absence of instruction emission is proven by exhaustive grep; what the flag should suppress is a user decision |
| `as_prompt_text` (Q5) | HIGH | Method read in full, with its own rustdoc calling it the dual-surface invariant |
| Gate warning inputs (Q6) | HIGH | Both `ToolInfo` types read; the annotation-dropping conversion located at `builder.rs:1223-1232` |
| Testing geometry (Q7/Q10) | HIGH | All four Makefile selectors read; both verify commands executed with pasted counts; the fuzz failure reproduced and pasted |
| Feature gating (Q8) | HIGH | Feature lists quoted; tripwire assertions read; the wasm composition reasoned from the two `#[cfg]` lines |
| Example (Q9) | HIGH | Collision found by reading the manifest and listing `examples/` |
| Pitfalls (Q11) | HIGH | PMAT baseline measured; lint targets read; SATD grep read |

**Research date:** 2026-09-02
**Valid until:** 2026-10-02 (30 days) — with two earlier expiries: the SEP-2640 draft is an open PR that was last pushed 4 days before this research, so re-check it before any conformance-shaped decision; and the Phase 125 WR-* findings may be closed by an intervening phase.
