# Roadmap: PMCP SDK Extensions

## Milestones

| Milestone | Phases | Status | Detail |
|---|---|---|---|
| v1.0 MCP Tasks Foundation | — | ✅ shipped 2026-02-22 | [archive](milestones/v1.0-ROADMAP.md) |
| v1.1 Task-Prompt Bridge | — | ✅ shipped 2026-02-23 | [archive](milestones/v1.1-ROADMAP.md) |
| v1.2 Pluggable Storage Backends | — | ✅ shipped 2026-02-24 | [archive](milestones/v1.2-ROADMAP.md) |
| v1.3 MCP Apps Developer Experience | — | ✅ shipped 2026-02-26 | [archive](milestones/v1.3-ROADMAP.md) |
| v1.4 Book & Course Update | — | ✅ shipped 2026-02-28 | [archive](milestones/v1.4-ROADMAP.md) |
| v2.0 MCP Tasks for PMCP SDK | — | ✅ backfilled 2026-06-11 | [archive](milestones/v2.0-ROADMAP.md) |
| v2.1 – v2.4 (Tasks DX, config-only servers, governed Excel, Agents & Teams) | 82–111 | ✅ superseded / shipped | [history](milestones/pre-v2.7-ROADMAP-history.md) |
| v2.5 MCP Spec 2026-07-28 (v2) Support | 112–119 | ✅ shipped 2026-08-22 | [archive](milestones/v2.5-ROADMAP.md) |
| v2.6 AI-Package Portability | 120–124 | ✅ shipped 2026-08-27 (tag `v2.19.1`) | [archive](milestones/v2.6-ROADMAP.md) |
| **v2.7 SEP-2640 Skills Conformance & Positioning** | **125–126+** | 🚧 **active** | below |

<sub>v2.1–v2.4 were never given their own per-milestone archive; their full phase detail lives in
`milestones/pre-v2.7-ROADMAP-history.md`, carried over verbatim at the v2.6 close so the collapse
of this file lost nothing.</sub>

---

## v2.7 SEP-2640 Skills Conformance & Positioning (Phase 125+)

**Milestone Goal:** Bring the shipped `skills` module (Phase 80, feature `skills`) into
conformance with the CURRENT SEP-2640 draft (PR #2640 head `sep/skills-extension`, rewritten
2026-08-29), then land the positioning work spikes 009-011 validated: workflow→skill projection,
digest-pinned agent skill consumption, and the tri-surface decision-matrix docs. Grounded in
validated spikes 008-011 (`.planning/spikes/`) and the spike-findings skill
(`.claude/skills/spike-findings-rust-mcp-sdk/` — references `sep-2640-conformance.md` and
`skills-positioning-tri-surface.md`).

**Scoping note:** Phase 125 opens the milestone (the CRITICAL conformance fix — the shipped
module declares a capability it cannot answer). Later phases (workflow→skill projection
`as_skill()`, `[[skills]]` digest pins on AgentPackage, decision-matrix docs) are to be added
via `/gsd-phase add` as they are taken up — implementation-order items 23-25 in the
spike-findings skill.

- [x] Phase 125: SEP-2640 Conformance — skills/list + skills/get — 5/5 plans, completed 2026-09-02
- [ ] Phase 126: Workflow→skill projection — `SequentialWorkflow::as_skill()` (spike 009, implementation-order item 23)

> **Ticked by hand, 2026-09-02.** `gsd_run query phase.complete 125` returned
> `roadmap_updated: false` and did not tick this box: it looks for the phase inside the
> "Current Milestone" section, which is v2.6 (phases 120-124), while this phase lives under
> the v2.7 heading. The same scoping mismatch made it return `next_phase: "124"`. Full
> account in `.planning/STATE.md` § Current Position.

### Phase 125: SEP-2640 Conformance — skills/list + skills/get

**Goal**: A pmcp server that declares `io.modelcontextprotocol/skills` actually answers it. The current SEP-2640 draft makes `skills/list` and `skills/get` mandatory for any server declaring the extension; the shipped module auto-declares and implements neither, so a conforming host's first `skills/list` call gets -32601. This phase routes both methods via the crate-private `InternalClientRequest` classifier (pattern at `src/types/protocol/mod.rs:583` — NO new public `ClientRequest` variant, 2.x promise), answers them entirely from the shipped `Skills` registry with conforming entries (verbatim frontmatter JSON + complete `{uri, digest: sha256, size}` manifests), retires (or legacy-gates) the nonstandard `skill://index.json`, validates name-identity (final URI segment == frontmatter `name`) at build, and guards the ≤512-file / ≤16 MiB limits at `into_handler()`.
**Depends on**: Nothing in-repo (first phase of the milestone). Spike 008
(`.planning/spikes/008-sep-2640-drift-check/`) is the measured drift evidence; the fix contract
lives in `.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md`.
**Requirements**: TBD — no formal REQ-IDs; the tracked requirement set is `125-CONTEXT.md`
decisions D-01..D-11. All 11 are covered by the plans below, verified by extracting the
gate-scanned regions (`<objective>`, `<action>`, `<behavior>`, `<read_first>`, `<verify>`,
`<acceptance_criteria>`, `<done>`, and frontmatter `must_haves`) from each PLAN and matching
D-NN citations: D-01 (01,05), D-02 (01,03,04), D-03 (04), D-04 (01), D-05 (01,03), D-06
(02,04,05), D-07 (01,02), D-08 (04), D-09 (01,05), D-10 (05), D-11 (01,05).

> **Corrected 2026-09-02.** This line previously read "gate `check.decision-coverage-plan`
> reports 11/11". That is **false as measured**: the gate returns `covered: 0` for this
> phase — and also for the **shipped** Phase 123 (`total: 16, covered: 0`), whose plans cite
> D-NN throughout. It is a pre-existing gate defect on this project, in the same family as
> the plan-phase gate false positives already recorded in project memory, not a signal about
> these plans. Do not read a future `covered: 0` from that verb as evidence of a planning
> gap here; use the manual extraction above, which is what the enumeration reflects.
**Plans:** 5/5 plans executed across 4 waves — 1:{01} 2:{02,03} 3:{04} 4:{05}

Plans:
**Wave 1**

- [x] 125-01-PLAN.md — TRACER: `skills/list` end-to-end over streamable HTTP + the SC#1 routing guarantees

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 125-02-PLAN.md — `skills/get` with draft-correct -32602 semantics, proven auth-before-params gate ordering, and the measured `ServerCore` method boundary *(rescoped 2026-09-02 by the cross-AI replan: the original "twin-site parity" claim was unbuildable — `ProtocolHandler::handle_request` accepts only the typed public `Request`, so `ServerCore` skills delegates would be dead code. Follows the Phase-112 `server/discover` precedent; see the 125-02 R-13 disposition.)*
- [x] 125-03-PLAN.md — Complete `resources` manifests, verbatim frontmatter, D-02 warn+exclude, name-identity reject, SEP limits warning

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 125-04-PLAN.md — Retire `skill://index.json` (12 tracked sites) + examples and the four documentation surfaces

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 125-05-PLAN.md — `make test-skills` gate leg with a zero-test-count guard, fuzz target, and the rustdoc deferral record

**Success Criteria** (what must be TRUE):

  1. `serde_json::from_value::<ClientRequest>` on `{"method":"skills/list"}` still returns Err
     (no new public variant — 2.x exhaustive-enum promise), yet a built server with registered
     skills answers `skills/list` and `skills/get` wire requests with entries carrying verbatim
     frontmatter JSON and complete `{uri, digest: sha256:{64 lowercase hex}, size}` manifests
     (spike 008 gaps #1, #2).
  2. `skill://index.json` is no longer served by default once `skills/list` lands — retired or
     behind an explicit legacy gate (gap #3).
  3. `Skills` build-time validation rejects a skill whose final URI segment does not equal its
     frontmatter `name`, and warns when a skill exceeds 512 files or 16 MiB (gaps #4, #5).
  4. Existing conforming behavior is unchanged: SKILL.md + supporting files remain byte-identical
     via `resources/read`, supporting files stay out of `resources/list`, the dual-surface prompt
     fallback still holds byte-equality, and examples s44/c10 still pass.
  5. `resources/directory/read` (gap #6) and client wrappers (gap #7) are explicitly deferred or
     implemented — never silently dropped; the current `{}` declaration legitimately means
     `directoryRead: false`.

### Phase 126: Workflow to skill projection — `SequentialWorkflow::as_skill()`

**Goal**: A `SequentialWorkflow` and its projected skill are the SAME content rendered twice, and
cannot drift — because the SDK owns the renderer. `SequentialWorkflow::as_skill()` (or
`Skills::from_workflow(&wf)`) derives a SEP-2640-conforming skill from the workflow's already-public
introspection surface, targeting the CURRENT entry shape (digests included) from day one, so the
Phase 125 conformance work is inherited rather than re-litigated.

**Depends on**: Phase 125 (the conforming `Skills::entries()` / `skills/list` surface this projects
INTO). Grounded in spike 009 (`.planning/spikes/`, VALIDATED) and
`.claude/skills/spike-findings-rust-mcp-sdk/references/skills-positioning-tri-surface.md`
(implementation-order item 23).

**Requirements**: No formal REQ-IDs — v2.7 has no REQUIREMENTS.md (`/gsd-new-milestone` writes one),
and the underlying requirement source is the MANIFEST bullets quoted in the tri-surface reference.
**The tracked requirement set for planning and verification is the six Success Criteria below,
carried as `SC-1`..`SC-6` in each plan's `requirements:` frontmatter**, plus the `126-CONTEXT.md`
decisions `D-01`..`D-16` with the three amendments `D-04a` / `D-15a` / `D-16a`.

Coverage as planned (verified by extracting the gate-scanned regions from each PLAN and matching
citations): SC-1 (01, 04, 07) · SC-2 (02, 05, 06, 07) · SC-3 (02, 06, 07) · SC-4 (01, 03, 07) ·
SC-5 (02, 05, 07) · SC-6 (04, 07). D-01 (01, 04) · D-02 (01, 04) · D-03 (01) · D-04 (01, 05, 07) ·
D-04a (01, 05, 07) · D-05 (all) · D-06 (all) · D-07 (04, 07) · D-08 (04) · D-09 (04) · D-10 (04, 06)
· D-11 (01, 02, 06, 07) · D-12 (02, 06) · D-13 (01, 02, 03, 06) · D-14 (01, 02, 06, 07) · D-15 (01,
02, 04, 06, 07) · D-15a (01) · D-16 (07) · D-16a (07).

> **CORRECTED 2026-09-03 (replan): the decision-coverage verb is NOT defective — the earlier
> `covered: 0` readings were a CALLER error.** `check.decision-coverage-plan` takes TWO POSITIONAL
> arguments, `PHASE_DIR` then `CONTEXT_PATH` (see `workflows/plan-phase.md:1361`). Invoked with only
> a context path it reports `total: 17, covered: 0` because it scanned no plan directory — and its
> own message says so. Invoked correctly it returns **`passed: true, total: 17, covered: 17`** for
> this phase, measured this session:
> `gsd_run query check.decision-coverage-plan "$PHASE_DIR" "$PHASE_DIR/126-CONTEXT.md"`.
> The manual enumeration above stands as documentation, but the gate is a working oracle and should
> be trusted over prose. Anyone re-testing the Phase 123 / 125 claim should re-run it with both
> positional arguments before repeating "pre-existing gate defect".

**Spec-less probe fallback: SKIPPED, recorded visibly.** There is no `126-SPEC.md`, so
`## Edge Coverage` and `## Prohibitions` were both absent; the fallback derives predicates from
requirement TEXT, and this phase has no requirement IDs to derive from. **No probe-derived
predicates were generated.** The SC set above and `126-VALIDATION.md`'s Per-Task Verification Map
are the edge set instead.

**Why this is a projection and not a rival feature.** Spike 011 measured the positioning as
*alongside, composing — never instead*: same tools and same data, the ineligible order got refunded
ONLY by the workflow surface. Context cost is NOT monotone. So the generated skill body must stay a
COMPLETE manual procedure, and its closing "Server-accelerated alternative" section
**cross-references the workflow prompt but never redirects to it**.

**The mapping (spike 009, ~90 lines, needs zero new SDK introspection).** The surface is already
public — `SequentialWorkflow::{name, description, arguments, steps, instructions}`,
`WorkflowStep::{name, tool, arguments, binding, guidance}`, `DataSource::{PromptArg, StepOutput,
Constant}`:

- frontmatter ← workflow name (**slugified by the projection**) + description
- Context ← workflow-level instructions — render `PromptContent::Text` properly
  (Debug-formatting it was spike 009's only real bug)
- Inputs ← argument specs
- Procedure ← per step: tool + rendered `DataSource` argument mappings + "Save the result as {binding}"
- "Judgment:" lines ← `with_guidance` text
- Closing "Server-accelerated alternative" ← names the prompt; the manual procedure stays complete

**Plans:** 6/7 plans executed across 5 waves — 1:{01} 2:{02,03} 3:{04,05} 4:{06} 5:{07}

Plans:

**Wave 1**

- [x] 126-01-PLAN.md — D-03 module split (alone, first) + TRACER: one trivial workflow projects, registers, and reads back byte-identical, with **safely encoded YAML frontmatter** and its round-trip proof *(carries the phase's two one-way decision gates: `build()`'s return shape, and builder-level reachability of the D-04a opt-in)*

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 126-02-PLAN.md — Full render breadth (Context / Inputs / per-step detail, sorted template bindings) + the SC-2 / SC-3 / SC-5 unit and property suite
- [x] 126-03-PLAN.md — SC-4 on the wire (conforming `skills/list` entry + byte-identical `resources/read`) + the ALWAYS/FUZZ target, its `[[bin]]` stanza, CI matrix row and registration tripwire

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 126-04-PLAN.md — `SkillProjection` builder, public `ProjectionWarning`, and the SC-6 gate warning *(records the D-02 return-shape deviation and the D-10 narrowing: SC-6 warnings have ONE delivery channel, `build()`'s structured return)*
- [x] 126-05-PLAN.md — D-04a opt-in prompt prepend on **both** handler kinds (rendered once, validation preserved), builder-level reachability, and transcript tests placed where `make test-skills` can reach them

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 126-06-PLAN.md — The D-14 golden + its CHANGELOG-forcing failure message, the module documentation contract, and both doctest legs

**Wave 5** *(blocked on Wave 4 completion)*

- [ ] 126-07-PLAN.md — `s56_workflow_skill_projection` example (asserts, not prints) + `deferred-items.md` + the full gate and the three verifications it structurally cannot run

**Success Criteria** (what must be TRUE):

  1. **Slugification is owned by the projection.** A workflow named `refund_flow` projects to skill
     `refund-flow`, and the final URI segment equals the frontmatter `name` — so the projection
     satisfies Phase 125's name-identity build check by construction rather than by luck.
  2. **Determinism.** Re-deriving the same workflow yields a byte-equal body. Asserted, not assumed.
  3. **Full coverage.** Every workflow fact — name, description, each argument, each step's tool and
     argument bindings, each `with_guidance` line — appears in the derived text, asserted
     string-by-string rather than by a length or substring heuristic.
  4. **Registry pass-through.** The projected skill registers and `skill://{name}/SKILL.md` reads
     back byte-identical through the real handler, carrying a conforming SEP-2640 entry (verbatim
     frontmatter + complete `{uri, digest: sha256, size}` manifest) — i.e. it satisfies Phase 125's
     contract on the wire, not just in-process.
  5. **The dual-surface invariant holds for free:** `as_prompt_text() == body`, and both surfaces
     name exactly the same tools (surface equivalence).
  6. **Projection-time warning on the workflow surface's blind spot.** Server-side execution runs
     every deterministic step regardless of guidance prose, so a **side-effecting step carrying
     gate-like guidance** must produce a warning at projection time — the one thing the projection
     can see that the executing surface cannot.

## Progress — v2.7 Milestone

*Milestone **v2.7 SEP-2640 Skills Conformance & Positioning** — opened 2026-09-01 with Phase 125.*

| Phase | Requirements | Plans Complete | Status | Completed |
|-------|--------------|----------------|--------|-----------|
| 125. SEP-2640 Conformance — skills/list + skills/get | D-01..D-11 (`125-CONTEXT.md`; no formal REQ-IDs) | 5/5 | Complete | 2026-09-02 |
| 126. Workflow→skill projection (`as_skill()`) | SC-1..SC-6 (ROADMAP) + D-01..D-16, D-04a/D-15a/D-16a (`126-CONTEXT.md`); no formal REQ-IDs | 6/7 | In Progress|  |

**Phase 125 close-out record (2026-09-02).** All five ROADMAP Success Criteria above verified
(`125-VERIFICATION.md`, status `passed`). UAT 3/3 passed (`125-UAT.md`) — three human decisions:
CR-01 accepted as a D-01-scoped residual risk, WR-06 shipped as-is with the capability guard
retained, and the remaining code-review findings carried forward. Security verified
(`125-SECURITY.md`, `threats_open: 0`; 23 threats, all closed, ASVS L1). Two items leave the phase
open by decision rather than by oversight, both in `deferred-items.md`:

- **CR-01 / T-125-21** — a server that declares `io.modelcontextprotocol/skills` and runs on
  **stdio** tears down the session on `skills/list`, silently (no JSON-RPC error to the client;
  `crate::log` is a no-op stub). Accepted: the SDK targets remote streamable-HTTP servers, and
  `skills` is opt-in — absent from both `default` and `full`.
- **Three code-review findings** — WR-03 (a build-time `panic!` inside a `Result`-returning
  `build()`, `src/server/builder.rs:1501`), WR-04 partial (the log-injection mitigation landed
  but has no test), WR-05 (the two middleware-path skills assemblers have no test coverage).

**⚠ This milestone is not the one the tooling tracks.** `.planning/STATE.md` and
`.planning/state.json` both still read `milestone: v2.6`, so every milestone-scoped verb —
`init.progress`, `phase.complete`, the progress counters — cannot see Phase 125. That is why
`phase.complete 125` returned `next_phase: "124"` and `roadmap_updated: false`, and why this
table and the checkbox above were written by hand. Resolving it means closing v2.6, which first
needs Phase 124's record finished (plans 06/07 have no SUMMARY though the release shipped as
`v2.19.2`/`v2.19.3`). Full account in `.planning/STATE.md` § Current Position.

**Next phases** are unplanned by design: the milestone's scoping note says the positioning work
(workflow→skill projection `as_skill()`, `[[skills]]` digest pins on AgentPackage, the
tri-surface decision-matrix docs — implementation-order items 23-25 in the spike-findings skill)
is added via `/gsd-phase add` as it is taken up.

---

## Backlog

Parking lot for unsequenced ideas. Items here aren't scheduled — promote with `/gsd:review-backlog` when ready.

### Phase 999.1: Delete DEFAULT_PROTOCOL_VERSION constant and make callsites explicit (BACKLOG)

**Goal:** Remove the public `DEFAULT_PROTOCOL_VERSION` re-export and replace each of its ~15 callsites with an explicit choice — either `LATEST_PROTOCOL_VERSION` (where the code is advertising what this SDK supports) or a literal `"2025-03-26"` with a `// backward-compat fallback` comment (where the code genuinely wants the widest-compatible version for un-negotiated peers). The name `DEFAULT` is misleading: nothing is actually "default" about it — it's a specific compat choice that happens to be older than `LATEST`, and that distinction is invisible at every callsite.

**Scope:**

- Breaking API change (public re-export at `src/lib.rs:307` + `src/types/mod.rs:32`) — requires minor version bump and release note
- Callsites to audit and convert:
  - `src/types/protocol/mod.rs:32` — `impl Default for ProtocolVersion`
  - `src/server/streamable_http_server.rs` lines 560, 971, 979, 981, 985, 1225, 1231, 1233, 1236
  - `src/server/core.rs:1329` and `:1376`
  - `src/shared/event_store.rs:367`
  - `src/lib.rs:286` — public doctest asserting the value
  - `src/types/protocol/version.rs:51,65` — unit tests
  - `benches/comprehensive_benchmarks.rs:421`

**Why:** Phase 65 simplify review flagged the inconsistency — `LATEST_PROTOCOL_VERSION = "2025-11-25"` but `DEFAULT_PROTOCOL_VERSION = "2025-03-26"`. Mechanically bumping `DEFAULT` to match `LATEST` would be a silent behavior change for peers reaching the fallback path (they'd be assumed to speak the newer protocol before they've said so). The right fix is to delete the misleading abstraction, not flip its value.

**Requirements:** TBD

**Plans:** 11/11 plans complete

Plans:

- [ ] TBD (promote with `/gsd:review-backlog` when ready)

### Phase 999.2: TOON data format feasibility and SDK integration (BACKLOG)

**Goal:** Investigate whether PMCP should add built-in support for TOON as an alternative wire format to JSON for MCP tool outputs and resource payloads, specifically to optimize performance of MCP Apps that often ship large JSON payloads. Deliver a spike report + recommendation (adopt / pilot / reject), and if adoption is recommended, a follow-up implementation plan for a feature-gated `toon` format with encoder/decoder integrated into `Content`, tool output serialization, and the MCP App bridge so servers and Apps can opt in with a single flag.

**Motivation:**

- MCP Apps frequently serialize large structured payloads (tables, datasets, chart data) — the dataviz, hotel gallery, and venue map examples are all 10–100 KB of JSON per response
- Both the LLM context window and the UI widget render path benefit from smaller payloads: fewer tokens consumed by tool output, faster postMessage bridge transfer, faster widget mount
- TOON (Token-Oriented Object Notation) is designed explicitly for this use case — schema-aware compression that encodes repeated keys and types once, yielding ~30–60% size reductions on tabular data compared to JSON
- If adoption works, MCP servers would flip a flag per-tool or per-resource to switch output format, and MCP Apps would transparently decode on the bridge side

**Research questions (spike scope, not implementation):**

1. Maturity of TOON — is the spec stable enough to commit a feature-gated SDK integration? Is there a Rust encoder/decoder crate, or would PMCP need to author one?
2. Compatibility — can TOON payloads ride over existing MCP `Content` variants (probably via a new `TextContent` MIME type or a new `Content::Toon` variant), or does it need protocol changes that break v2025-11-25 compat?
3. Measurement — what are the realistic size/token savings on representative MCP App payloads (dataviz, gallery, map from existing examples)? Do LLMs tokenize TOON efficiently, or does the token savings on the wire get lost when Claude/GPT re-tokenize the decoded content?
4. Widget-side decoder — can the MCP App bridge (TypeScript) decode TOON in-browser without a heavy dependency, or is this a non-starter for the WASM/iframe sandbox?
5. Opt-in UX — what does `#[mcp_tool(output_format = "toon")]` or equivalent look like on the server side? What's the per-call server-side negotiation story?

**Why backlog (not an active phase):**

- The user's framing is explicitly exploratory ("let's investigate if we can add it as a built-in support")
- TOON is a newer format — the feasibility spike should happen before committing a phase slot
- The v2.1 rmcp Upgrades milestone is scoped to documentation polish; runtime data-format work doesn't belong there
- Natural home after spike: either seed of a "v2.2 Payload Optimization" milestone or early phase of a milestone focused on MCP Apps performance

**Promotion path:** Run `/gsd:discuss-phase 999.2` to gather context, then `/gsd:research-phase 999.2` for the spike, then promote via `/gsd:review-backlog` into an active milestone with a concrete Phase N number.

**Requirements:** TBD (depends on spike outcome)

**Plans:** 6 plans

Plans:

- [ ] TBD (promote with `/gsd:review-backlog` when ready)

### Phase 69: rmcp parity research — ergonomics gap analysis + follow-on phase proposals

**Goal:** Produce a rigorous, evidence-backed gap matrix comparing pmcp vs rmcp on *ergonomics* (macro DX, builder APIs, typed wrappers, handler shapes, state/extra patterns) and use it to propose 2–4 concrete follow-on phases to close the credibility/DX gap. Transports, examples polish, and docs coverage are intentionally out of scope — Phase 68 handles those surfaces at the polish layer.

**Deliverables:**

- `69-RESEARCH.md` — gap matrix (per-feature: rmcp approach, pmcp approach, gap severity, evidence citations)
- `69-PROPOSALS.md` — 2–4 follow-on phase proposals with goals, scope, and rough success criteria, ready to slot into v2.1 or seed v2.2

**Requirements**: TBD (derived from research findings; expected to seed new v2.1/v2.2 requirement IDs)
**Depends on:** Phase 68
**Plans:** 3 plans

Plans:

- [x] 69-01-PLAN.md — Produce the rmcp vs pmcp ergonomics gap matrix (69-RESEARCH.md) across 6 surfaces
- [x] 69-02-PLAN.md — Derive follow-on phase proposals from High-severity gaps (69-PROPOSALS.md)
- [x] 69-03-PLAN.md — Quality gate + land PARITY-* requirement IDs + update STATE/PROJECT

### Phase 70: Add Extensions typemap and peer back-channel to RequestHandlerExtra (PARITY-HANDLER-01)

**Goal:** Extend `RequestHandlerExtra` with two drop-in additive capabilities — a typed-key `Extensions` map (HANDLER-02) for request-scoped user data crossing middleware/handler boundaries, and an optional `PeerHandle` back-channel (HANDLER-05) exposing `sample` / `list_roots` / `progress_notify` from inside tool/prompt/resource handlers — without breaking any existing `::new(...)` or `::with_session(...)` call site. Restructured from 3 plans to 4 plans after cross-AI review (70-REVIEWS.md) + codebase verification (70-REVIEW-VERIFICATION.md) confirmed 5 of Codex's HIGH findings: the original plan set assumed an outbound `ServerRequest` transport + response-correlation layer that does not exist in the live codebase. Plan 02 (NEW) builds that foundational plumbing before Plan 03 wires the peer.
**Requirements**: PARITY-HANDLER-01
**Depends on:** Phase 69
**Plans:** 4 plans

Plans:

- [x] 70-01-PLAN.md — Extensions typemap on both RequestHandlerExtra structs + #[non_exhaustive] + accessor parity + 5 proptests + refactor 12 struct-literal test sites (Wave 1)
- [x] 70-02-PLAN.md — ServerRequestDispatcher (outbound ServerRequest + response correlation) + Server::run drain-to-transport + route TransportMessage::Response through dispatcher (NEW plan from reviews replan — addresses Codex Findings 2+3) (Wave 2)
- [x] 70-03-PLAN.md — PeerHandle trait + DispatchPeerHandle delegating to Plan 02 dispatcher + conditional .with_peer(...) at 9 dispatch sites + dispatch-path round-trip integration test (Wave 3)
- [x] 70-04-PLAN.md — Examples s42 + s43 (s43 uses real ToolHandler per Codex Finding 5) + fuzz target + rustdoc migration prose with explicit semver posture + make quality-gate (Wave 4)

### Phase 71: Rustdoc fallback for #[mcp_tool] tool descriptions (PARITY-MACRO-01)

**Goal:** Enable `#[mcp_tool]` to harvest the attached function's rustdoc as the tool description when the `description = "..."` attribute is omitted — eliminating forced duplication where a well-documented tool fn must repeat its description in both the rustdoc block and the macro attribute. Preserves precedence (explicit attribute wins over rustdoc), fails with a clear error when neither is present, and remains backwards-compatible with all existing call sites. Derived from 69-PROPOSALS.md Proposal 3 (MACRO-02, High severity).
**Requirements**: PARITY-MACRO-01
**Depends on:** Phase 70
**Plans:** 4/4 plans complete

Plans:

- [x] 71-01-PLAN.md — Create new sibling crate `crates/pmcp-macros-support/` (non-proc-macro) holding the pure `extract_doc_description` normalization helper with unit tests + proptest invariants — resolves HIGH-1 via Option A so proc-macro crate API restrictions don't block property/fuzz consumers (Wave 1)
- [x] 71-02-PLAN.md — `pmcp-macros` adds path dep on `pmcp-macros-support` + single shared `resolve_tool_args` resolver in `mcp_common.rs`; both parse sites (`mcp_tool.rs` standalone + `mcp_server.rs::parse_mcp_tool_attr` impl-block) delegate to it; integration tests lock symmetry (MEDIUM-1) (Wave 2)
- [x] 71-03-PLAN.md — 4 trybuild compile-fail snapshots (existing regenerated + new empty-args + new non-empty-args + regenerated multi-args) + README migration section with Limitations subsection + mixed-shape fuzz target `rustdoc_normalize.rs` (MEDIUM-2 + MEDIUM-3 + LOW-3) (Wave 3)
- [x] 71-04-PLAN.md — Workspace `pmcp`-dependency ripple audit + version bumps (pmcp 2.3.0→2.4.0 MINOR per MEDIUM-4, pmcp-macros 0.5.0→0.6.0, new pmcp-macros-support 0.1.0, concurrent downstream patch bumps cargo-pmcp 0.6.0→0.6.1 + mcp-tester 0.5.0→0.5.1 per CLAUDE.md §"Version Bump Rules") + CHANGELOG entry + REQUIREMENTS.md closure + `make quality-gate` (HIGH-2 + MEDIUM-4) (Wave 4)

### Phase 72: Investigate rmcp as foundations for pmcp - evaluate using rmcp for protocol level while focusing pmcp on pragmatic batteries-included SDK for enterprise use cases ✓ COMPLETE

**Status:** COMPLETE (2026-04-19) — **Recommendation: D** (Maintain pmcp as authoritative Rust MCP SDK; do not migrate onto rmcp). 7/9 decision thresholds resolved; T6/T7 remain UNKNOWN per 72-CONTEXT.md. Slice 1 spike executed — serde `params: null` round-trip fails against rmcp 1.5.0, downgrading inventory row 1 from EXACT to compatible-via-adapter. Phase 69's parity phases (70, 71, CLIENT-02) remain the forward path. See `.planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-RECOMMENDATION.md`.

**Goal:** Produce a research/decision-only recommendation on whether pmcp's protocol layer should be refactored to sit on top of rmcp 1.5.0 — repositioning pmcp + mcp-tester + mcp-preview + cargo-pmcp as a pragmatic, batteries-included, enterprise-focused SDK built *on top of* rmcp rather than alongside it. Deliverables are 7 markdown documents (CONTEXT, inventory, strategy matrix, PoC proposal, PoC results, decision rubric, final recommendation). If the recommendation is adopt (A/B/C1/C2), migration itself is scoped as a separate future v3.0 phase; if stay (D), Phase 69's parity phases remain the path forward.
**Requirements**: RMCP-EVAL-01, RMCP-EVAL-02, RMCP-EVAL-03, RMCP-EVAL-04, RMCP-EVAL-05
**Depends on:** Phase 71
**Plans:** 3/3 plans complete

Plans:

- [x] 72-01-PLAN.md — Seed RMCP-EVAL-01..05 in REQUIREMENTS.md; produce 72-INVENTORY.md (inversion inventory, >=15 pmcp module families with file:line + rmcp evidence) and 72-STRATEGY-MATRIX.md (5 options x 5 criteria = 25 cells, no TBD) (Wave 1)
- [x] 72-02-PLAN.md — Produce 72-POC-PROPOSAL.md (3 slices, each <=500 LOC, at least one <=3 days, with LOC/Files/Pass/Fail/Time-box fields) and 72-DECISION-RUBRIC.md (>=5 falsifiable thresholds, each followed by Data source) (Wave 2)
- [x] 72-03-PLAN.md — Produce 72-RECOMMENDATION.md (RMCP-EVAL-05) — opens with `**Recommendation:** <A|B|C|D|E>`, contains 5 per-criterion justification subsections citing T-IDs + inventory/matrix rows, lists UNRESOLVED thresholds, and names the next-phase handoff (Wave 3)

### Phase 72.1: Finalize landing support (INSERTED)

**Goal:** Ship CR-03 rev-2 — replace build-time `NEXT_PUBLIC_*` env vars in the landing Next.js template with a runtime `fetch('/landing-config')` via a new required shared hook `useLandingConfig`, fix 3 stale rustdoc references in `cargo-pmcp/src/landing/config.rs`, and bump `cargo-pmcp` 0.8.0 -> 0.8.1 (patch, additive). Unblocks pmcp.run Phase 71 UAT Test 7 and Cost Coach production launch.
**Requirements**: LAND-CR03-01
**Depends on:** Phase 72
**Plans:** 1/1 plans complete

Plans:

- [x] 72.1-01-PLAN.md — Create `lib/useLandingConfig.ts` hook; rewrite 4 consumers (signup, callback, connect [server->client flip], Header [conditional button]); fix 3 rustdoc comments in `src/landing/config.rs`; bump `Cargo.toml` 0.8.0 -> 0.8.1; run `make quality-gate` + `cargo doc` + template `tsc`/`next build` + grep guardrails G1..G6 + manual AC-11 offline gate (Wave 1)

### Phase 74: Add cargo pmcp auth subcommand with multi-server OAuth token management

**Goal:** Consolidate OAuth handling for cargo-pmcp's server-connecting commands into a dedicated `auth login/logout/status/token/refresh` command group with a per-server-keyed token cache. Add SDK-level Dynamic Client Registration (RFC 7591) so any PMCP-built client can auto-register, and expose it via a `--client <name>` flag on `auth login` for testing pmcp.run's client-branded login pages.
**Requirements**: SDK-DCR-01, CLI-AUTH-01
**Depends on:** Phase 72.1
**Plans:** 3/3 plans complete

Plans:

- [x] 74-01-PLAN.md — SDK DCR: OAuthConfig refactor (client_id Option), DcrRequest/DcrResponse re-export, auto-fire DCR in OAuthHelper, unit/property/fuzz/mockito-integration tests, examples/c08_oauth_dcr.rs, CHANGELOG entry (Wave 1, pmcp crate)
- [x] 74-02-PLAN.md — CLI auth group: new commands/auth_cmd/ module (login/logout/status/token/refresh + TokenCacheV1 cache with atomic writes & URL normalization), main.rs wiring, resolve_auth_middleware cache fallback with near-expiry auto-refresh, pentest.rs migration to shared AuthFlags, tempfile promoted to regular dep, mockito+cli integration tests (Wave 2, cargo-pmcp crate)
- [x] 74-03-PLAN.md — Release coordination: bump pmcp 2.4.0→2.5.0 and cargo-pmcp 0.8.1→0.9.0, update cargo-pmcp pmcp dep pin to 2.5.0, finalize CHANGELOG date, run make quality-gate to match CI exactly (Wave 3)

### Phase 73: Typed client helpers + list_all pagination (PARITY-CLIENT-01)

**Goal:** Ship additive, non-breaking `Client` ergonomics (pmcp 2.6.0): four typed-input helpers (`call_tool_typed`, `call_tool_typed_with_task`, `call_tool_typed_and_poll`, `get_prompt_typed`), four auto-paginating list helpers (`list_all_tools`, `list_all_prompts`, `list_all_resources`, `list_all_resource_templates`) with a bounded `max_iterations` safety cap, and a new `ClientOptions` config struct (`#[non_exhaustive]`) wired through a new `Client::with_client_options` constructor. Closes the client-side rmcp-parity DX gap (PARITY-CLIENT-01).
**Requirements**: PARITY-CLIENT-01
**Depends on:** Phase 74
**Plans:** 3/3 plans complete

Plans:

- [x] 73-01-PLAN.md — ClientOptions scaffold + new Client::with_client_options constructor + four typed helpers (call_tool_typed / _with_task / _and_poll / get_prompt_typed) with doctests, unit tests, and one property test (Wave 1, pmcp crate)
- [x] 73-02-PLAN.md — Four list_all_* auto-paginating helpers with max_iterations cap enforcement (T-73-01 DoS mitigation); integration test file tests/list_all_pagination.rs; two property tests (flat-concatenation + cap-enforcement); new fuzz target fuzz/fuzz_targets/list_all_cursor_loop.rs (Wave 2, pmcp crate)
- [x] 73-03-PLAN.md — Release coordination: examples/c09_client_list_all.rs (avoids c08 collision) + examples/c02_client_tools.rs update + README index; bump pmcp 2.5.0→2.6.0 across all 8 pin lines in 7 Cargo.toml files; CHANGELOG v2.6.0 entry; REQUIREMENTS.md §55 D-15 doc-fix (call_prompt_typed → get_prompt_typed); README Key Features bullet; make quality-gate (Wave 3)

### Phase 75: Fix PMAT issues

**Goal:** Restore the auto-generated `Quality Gate: passing` README badge by remediating PMAT findings (cognitive complexity is the gating dimension; SATD, duplicate, entropy, sections are best-effort within waves). After this phase, `pmat quality-gate --fail-on-violation --checks complexity` exits 0 and a CI gate prevents regression.
**Requirements**: None (quality-debt remediation; must_haves derived from CONTEXT.md decisions D-01..D-09)
**Depends on:** Phase 74
**Plans:** 6 plans

Plans:

- [x] 75-00-PLAN.md — Wave 0: Baseline + spike (PMAT path-filter empirical test, insta snapshot baseline for pmcp-macros, semantic regression baseline for pmcp-code-mode, PMAT version pin in CI) — completed 2026-04-23 — D-09 resolved (include_works=false, .pmatignore is the only honored filter), D-10 resolved D-10-B (PMAT ignores #[allow] — SCOPE EXPANSION DETECTED), D-11 resolved D-11-B (bare gate fails on 5 dimensions — Wave 5 must patch quality-badges.yml)
- [x] 75-01-PLAN.md — Wave 1: src/ + pmcp-macros/ refactors — completed 2026-04-24. 20 hotspots cleared to ≤25 via P1-P3 (zero P5 usage, zero escapees). Delta: PMAT complexity 94→75 (−19). Task 1a-C explicitly skipped per addendum Rule 1 (migrated to Phase 75.5 Category A); pre-existing bare #[allow] at streamable_http_server.rs:1004 removed. Macro expansion snapshots byte-identical (Wave 0 contract preserved).
- [x] 75-02-PLAN.md — Wave 2: cargo-pmcp/ refactors — completed 2026-04-24. 40 hotspots cleared to ≤25 via P1-P4 (zero P5 usage, zero escapees). Both monsters (check.rs::execute cog 105→≤25, handle_oauth_action cog 91→≤25) decomposed. Delta: PMAT complexity-gate 75→29 (−46); cargo-pmcp cog>25 40→0. `make quality-gate` exits 0. Shared scan_for_package helper established in cloudflare/init.rs (3-bird kill).
- [x] 75-03-PLAN.md — Wave 3: pmcp-code-mode/ refactors — completed 2026-04-24. All 5 named hotspots cleared to ≤25 via P6 (eval.rs) + P1 (policy_annotations.rs, schema_exposure.rs); zero P5 usage; zero escapees. Both eval-monsters decomposed: evaluate_with_scope 123→17, evaluate_array_method_with_scope 117→≤25. Delta: PMAT complexity-gate 29→22 (−7). Pre-existing pmcp-code-mode lint debt (18 lib + 28 test errors + 3 dead-code) cleared in opening sweep. `make quality-gate` exits 0. Wave 0 semantic-regression baseline byte-identical (34 passed throughout).
- [x] 75-04-PLAN.md — Wave 4: scattered crate hotspots + examples/fuzz handling per Wave 0 spike + SATD triage per D-04 + final pre-Wave-5 gate verification — completed 2026-04-25. 5 plan-named hotspots cleared to ≤25 (P1+P4) plus 8 additional warning-level cog 24-25 violations refactored under Rule 3 (gate-counted but out-of-plan-list). `.pmatignore` configured for fuzz/+packages/+examples/ per Wave 0 chosen_path: (a). 11 in-scope SATDs migrated to `// See #NNN` refs against 3 umbrella issues (paiml/rust-mcp-sdk#247/#248/#249); 14 SATD matches classified as out-of-D-04-scope scaffold/template content. Delta: PMAT complexity-gate 22→0 (−22); aggregate Phase 75 delta 94→0. `pmat quality-gate --fail-on-violation --checks complexity` exits 0. Wave 5 can flip the README badge.
- [~] 75-05-PLAN.md — Wave 5: D-07 enforcement (CI gate in ci.yml, regression-PR fail-closed test, badge-flip confirmation, CLAUDE.md docs update). NOTE (D-11-B): must also patch `.github/workflows/quality-badges.yml:~72` bare gate → `--checks complexity` per 75-ADDENDUM-D10B.md Rule 5. — substantially completed 2026-04-25. Tasks 5-01 (ci.yml gate + D-11-B quality-badges.yml alignment), 5-04 (CLAUDE.md docs) landed; Task 5-02 replanned mid-execution (option A → option B per user) — fork-internal CI did not fire due to fork-main divergence; switched to local-pmat empirical evidence (cog-77 fixture exits 1 by name) recorded in 75-05-GATE-VERIFICATION.md. Task 5-03 (badge flip on README) **deferred** until Wave 5 lands on `paiml/rust-mcp-sdk:main` — operator follow-up: trigger `gh workflow run quality-badges.yml -R paiml/rust-mcp-sdk` post-merge and append observation to GATE-VERIFICATION.md.

### Phase 75.5: PMAT ex-P5 refactor backlog

**Goal:** Absorb the refactor work that Phase 75 could not land under P5 (`#[allow(clippy::cognitive_complexity)]` + `// Why:`) because Wave 0 D-10 spike proved PMAT 3.15.0 ignores the allow attribute. Category A: the 12 pre-existing bare-allow sites in `src/` (orchestrator-verified count; PATTERNS.md said "13" but the 13th — `streamable_http_server.rs:1004 handle_post_with_middleware` — was already refactored in Phase 75 Wave 1a). Category B: escapees logged to `75.5-ESCAPEES.md` during Plans 75-01..75-04 — empty (zero entries logged across Phase 75 Waves 1-4). Each Category A site either refactors to ≤25 or has the ineffective `#[allow]` removed because the underlying function already simplified.
**Requirements**: None (quality-debt remediation, sibling of Phase 75)
**Depends on:** Phase 75 Waves 1-4 complete (so Category B is known — confirmed empty as of 2026-04-25). MAY land in parallel with Phase 75 Wave 5 or before it.
**Plans:** 1/1 plan complete

Plans:

- [x] 75.5-01-PLAN.md — Wave 1: 12 Category-A bare `#[allow(clippy::cognitive_complexity)]` attributes removed from src/ (server/, server/transport/, shared/, client/) — completed 2026-04-25. All 12 sites resolved by single-line attribute deletion (no refactor triggered — clippy pedantic+nursery quiet on `--features full` post-removal, confirming all underlying functions sit at cog ≤25). `make quality-gate` exit 0; `pmat quality-gate --fail-on-violation --checks complexity` exit 0 (PMAT 3.15.0); `grep -rn '#[allow(clippy::cognitive_complexity)]' src/` 0 matches. ESCAPEES.md (Category B) unchanged at 0 entries. Two pre-existing environmental test failures (mcp-e2e-tests::chess chromiumoxide browser archive missing; pmcp-tasks::store::redis/dynamodb Connection refused) classified out-of-scope per deviation-rules SCOPE BOUNDARY — neither touches src/server/, src/shared/, or src/client/. Commits: fae333fa (Task 1: server/+server/transport/), 7a0cc362 (Task 2: shared/+client/).

### Phase 76: cargo-pmcp IAM declarations — servers declare IAM needs in deploy.toml

**Goal:** Ship pmcp-run CR `CLI_IAM_CHANGE_REQUEST.md` in one phase — Part 1 adds a stable `McpRoleArn` CfnOutput (`Export.Name = pmcp-${serverName}-McpRoleArn`) to both generated CDK stack templates (pmcp-run + aws-lambda), unblocking bolt-on stacks via `Fn::ImportValue`. Part 2 adds an optional `[iam]` section to `.pmcp/deploy.toml` with three repeated tables (`[[iam.tables]]`, `[[iam.buckets]]`, `[[iam.statements]]`) that translate to `addToRolePolicy` calls on the Lambda execution role, plus a new `cargo pmcp validate deploy` subcommand that hard-errors on IAM footguns (Allow-*-*, bad effects, malformed actions). Backward compatible (empty default) per D-05 byte-identity. Target: cargo-pmcp 0.10.0 (additive minor bump).
**Requirements**: PART-1 (McpRoleArn export), PART-2 (declarative `[iam]` section + validator)
**Depends on:** Phase 75
**Plans:** 5/5 plans complete

Plans:

- [x] 76-01-PLAN.md — Wave 1: Part 1 — McpRoleArn CfnOutput in both template branches + `render_stack_ts` renderer extraction + D-03 aws-iam import fix + Wave 1 golden-file baseline (D-05 anchor)
- [x] 76-02-PLAN.md — Wave 2: Full IamConfig schema (TablePermission / BucketPermission / IamStatement) wired into DeployConfig with `skip_serializing_if` to preserve D-05 + serde roundtrip integration tests
- [x] 76-03-PLAN.md — Wave 3: Translation rules (`deployment/iam.rs::render_iam_block`) emitting D-02 4-action DynamoDB lists + S3 object-level ARNs + passthrough statements, wired into `render_stack_ts` via a single `{iam_block}` named placeholder + per-rule unit tests + 9 proptests
- [x] 76-04-PLAN.md — Wave 4: Validator (`validate` + `Warning`) enforcing 6 CR-locked hard-error rules + 2 warning classes + `ValidateCommand::Deploy` subcommand + DeployExecutor hook blocking deploy on hard errors + 29 new tests covering T-76-02 mitigation
- [x] 76-05-PLAN.md — Wave 5: `fuzz_iam_config` libfuzzer target + corpus seeds + `deploy_with_iam` runnable example + cost-coach fixture + DEPLOYMENT.md IAM Declarations section + README.md pointer + CHANGELOG 0.10.0 entry + version bump + final `make quality-gate`

### Phase 77: Add cargo pmcp configure commands

Developers using cargo pmcp across multiple deployment and upload targets (dev/prod, per-server) currently struggle to maintain and switch between environments. Design and implement `cargo pmcp configure` (modeled after `aws configure`) that lets a developer:

(1) define named targets (e.g., dev, prod, staging) with target-specific configuration: pmcp.run discovery endpoint URL (PMCP_API_URL like https://ipwojemcm6.execute-api.us-west-2.amazonaws.com or its /.well-known/pmcp-config variant), AWS CLI profile, region, and any target-specific credentials/secrets;

(2) switch quickly between targets with a per-workspace selection (one server can stay in dev mode pointing at a dev pmcp.run while a sibling server in the same monorepo deploys to prod);

(3) extend cleanly to non-pmcp.run target types: aws-lambda direct deploy with different AWS profiles, Google Cloud Run, or future targets;

(4) integrate with existing cargo pmcp deploy / cargo pmcp pmcp.run upload flows so they read the active target instead of hardcoded URLs/profiles.

Scope likely includes: a config schema (TOML in workspace .pmcp/ or user ~/.config/pmcp/), `cargo pmcp configure add|use|list|remove|show`, env var override support (PMCP_TARGET=name), and explicit precedence rules between workspace, user, and env.

**Goal:** Ship a `cargo pmcp configure` command group (add/use/list/show) that manages named deployment targets in `~/.pmcp/config.toml` and a per-workspace `.pmcp/active-target` marker; integrates with `cargo pmcp deploy` and `pmcp.run upload` via a precedence-merge resolver (ENV > flag > target > deploy.toml) and a fixed-order header banner; maintains zero-touch backward compatibility for users without a config.toml.
**Requirements**: REQ-77-01, REQ-77-02, REQ-77-03, REQ-77-04, REQ-77-05, REQ-77-06, REQ-77-07, REQ-77-08, REQ-77-09, REQ-77-10
**Depends on:** Phase 76
**Plans:** 9/9 plans complete

Plans:

- [x] 77-01-PLAN.md — Mint REQ-77-01..REQ-77-10 in REQUIREMENTS.md; bump cargo-pmcp 0.10.0 → 0.11.0; CHANGELOG stub
- [x] 77-02-PLAN.md — Rename existing deploy `--target` to `--target-type` (with alias); add new global `--target` named-target flag on Cli
- [x] 77-03-PLAN.md — Module skeleton + TargetConfigV1 schema (TOML, atomic write, 0o600) + workspace utility
- [x] 77-04-PLAN.md — `configure add` (interactive + flag-driven, raw-credential validator) + `configure use` (workspace marker)
- [x] 77-05-PLAN.md — `configure list` (text + stable JSON) + `configure show` (raw + merged-with-attribution placeholder)
- [x] 77-06-PLAN.md — Resolver (precedence walk, env injection helper) + banner (D-13 fixed-order, OnceLock idempotent) + show.rs enrichment
- [x] 77-07-PLAN.md — Top-level Cli wiring: register Configure variant, dispatch arm, env injection in main.rs, banner emission in deploy/mod.rs
- [x] 77-08-PLAN.md — Integration tests (full lifecycle, zero-touch, concurrent writes) + fuzz target + working multi-target-monorepo example
- [x] 77-09-PLAN.md — DRY cleanup (shared validate_target_name) + rustdoc audit + CHANGELOG date + `make quality-gate` certification + manual interactive UX checkpoint

### Phase 78: cargo pmcp test apps --mode claude-desktop: detect missing MCP Apps SDK wiring in widgets

Goal: Catch the silent-fail bug where a widget passes `cargo pmcp test apps` and renders fine in ChatGPT but breaks in Claude Desktop / claude.ai because the widget HTML never imports `@modelcontextprotocol/ext-apps`, never instantiates `App`, and never registers the four required handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`) before `connect()`.

Scope (this phase):

1. Promote `AppValidationMode::ClaudeDesktop` from placeholder ("same as Standard for now" at `crates/mcp-tester/src/app_validator.rs:28-29`) to a real strict mode.
2. In `cargo-pmcp/src/commands/test/apps.rs`, fetch each App-capable tool widget body via `resources/read` and pass `Vec<(uri, html)>` into the validator (keeps validator a pure function; ~30 LOC of plumbing).
3. Add static script-block checks behind `--mode claude-desktop`:
   - Imports `@modelcontextprotocol/ext-apps` OR has >=3 of the 4 protocol-handler property assignments (handles minified bundles where the import string is preserved but identifiers are renamed; both signals survive Vite singlefile minification).
   - Constructs `new App({...})` with non-empty Implementation.
   - Registers `onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror` (ERROR each).
   - Registers `ontoolresult` (WARN - some widgets render from `getHostContext().toolOutput`).
   - Calls `app.connect()` (ERROR).
   - "ChatGPT-only channels and no ext-apps wiring" -> ERROR in `claude-desktop` mode, OK in `chatgpt` mode.
4. Severity calibration matches existing pattern: `Standard` mode = WARN (MCP Apps is optional in the spec); `ClaudeDesktop` mode = ERROR - mirrors how `Standard` vs `ChatGpt` treat `openai/*` keys today.
5. Polish: error messages link to specific anchors in `src/server/mcp_apps/GUIDE.md` (especially the "Critical: register all four handlers before connect()" warning at line 185); update README and `cargo pmcp test apps --help` to document the new mode and recommend it as the pre-deploy check for servers shipping to Claude clients.

Out of scope (defer to a later phase):

- `PreviewMode::ClaudeDesktop` host emulator (postMessage init/tool-result/teardown simulation in `crates/mcp-preview/src/server.rs`). User wants to think about it later and may unify the preview UX across ChatGPT/Claude modes rather than add a third mode.

Reference / context:

- Proposal from the Cost Coach team: `/Users/guy/projects/mcp/cost-coach/drafts/proposal-pmcp-mcp-app-widget-validation.md`
- Failing widget bundle + working fix available from Cost Coach as a regression fixture (request via the proposal author).
- Verified state of the codebase: `AppValidationMode::ClaudeDesktop` is wired into Display/FromStr/CLI parsing but has zero behavior behind it; `AppValidator::validate_tools` only consumes `&[ResourceInfo]` metadata - no `resources/read` call, so widget HTML is never inspected.

ALWAYS requirements (per CLAUDE.md):

- Unit tests for each new check (positive and negative cases for each handler / SDK signal).
- Property tests for the script-block scanner (must not panic on arbitrary HTML/JS input; idempotent on normalized whitespace).
- Fuzz target for the regex/AST scan path.
- A working example: a `cargo run --example` (or fixture under `examples/`) showing a deliberately-broken widget that fails `--mode claude-desktop` and a corrected one that passes - same widget pair the Cost Coach team will provide.

Acceptance criteria:

- The Cost Coach reproducer (broken widget) FAILS `cargo pmcp test apps --mode claude-desktop` with errors that name the missing handler(s).
- The corrected version PASSES.
- `cargo pmcp test apps` (no flag, Standard mode) still passes for both - no regression for the permissive default.
- `--mode chatgpt` behavior unchanged.
- README + `--help` document the new mode.

**Goal:** Promote `AppValidationMode::ClaudeDesktop` from a placeholder to a real strict mode that statically inspects each App-capable widget HTML body (fetched via `resources/read`) for the `@modelcontextprotocol/ext-apps` import, the `new App({...})` constructor, the four required protocol handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`), and the `app.connect()` call — emitting ERROR (vs WARN in Standard mode) on missing signals so widgets shipping to Claude Desktop / Claude.ai are caught before deploy.
**Requirements**: PHASE-78-AC-1, PHASE-78-AC-2, PHASE-78-AC-3, PHASE-78-AC-4, PHASE-78-AC-5, PHASE-78-ALWAYS-UNIT, PHASE-78-ALWAYS-PROPERTY, PHASE-78-ALWAYS-FUZZ, PHASE-78-ALWAYS-EXAMPLE
**Depends on:** Phase 77
**Plans:** 7/11 plans executed (cycle-1 03/04 done; cycle-1 wave 4 plan 08 paused at checkpoint; cycle-2 plans 09-11 added 2026-05-02)

Plans:
**Wave 1**

- [x] 78-01-PLAN.md — Validator core: extend `AppValidator` with `validate_widgets`, regex-based scanner, mode-driven severity (Wave 1)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 78-02-PLAN.md — CLI plumbing: wire `read_widget_bodies` into `cargo pmcp test apps` (Wave 2)
- [x] 78-04-PLAN.md — Docs polish: README sections, `--help` long-text, GUIDE.md anchor expander (Wave 3, parallel with 78-03)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 78-03-PLAN.md — ALWAYS requirements: fixtures, property tests, fuzz target, working example (Wave 3)

### Phase 79: cargo pmcp deploy: widget pre-build + post-deploy verification (build half: auto-detect widget/ and widgets/ only, package-manager runner, generated build.rs with cargo:rerun-if-changed via env-var path resolution, [[widgets]] config with explicit embedded_in_crates, doctor checks; verify half: warmup grace + test check + conformance + apps --mode claude-desktop, on_failure=fail default; depends on Phase 78; out of scope: auto-rollback, multi-target)

**Goal:** Close two silent-failure gaps in `cargo pmcp deploy` proven by Cost Coach: (A) deploy ships stale `widget/dist/*.html` because nobody ran `npm run build`; (B) Cargo's incremental cache holds a stale `include_str!`-built binary; (C) widget JS SDK is misconfigured but deploy reports success because nothing probes the live endpoint. Build half auto-detects widget directories, runs the lockfile-determined package manager, sets `PMCP_WIDGET_DIR` for cache invalidation via a generated `build.rs`. Verify half runs warmup → check → conformance → apps lifecycle after Lambda hot-swap and surfaces a screaming-loud LIVE-but-broken banner on failure (`on_failure="fail"` default) with the manual rollback command pre-printed.
**Requirements**: REQ-79-01..18 (locally-derived per CONTEXT.md "Implementation Decisions"; phase has no numbered REQUIREMENTS.md entries)
**Depends on:** Phase 78
**Plans:** 7/7 plans complete

Plans:

- [x] 79-00-PLAN.md — Master plan: wave structure, requirement-to-plan mapping, version bumps, locked planner decisions
- [x] 79-01-PLAN.md — Wave 1: test fixtures + config schema (`WidgetsConfig`, `PostDeployTestsConfig`, `OnFailure`, `TestOutcome`)
- [x] 79-02-PLAN.md — Wave 2: widget pre-build orchestrator + `--no-widget-build` / `--widgets-only` CLI flags + `PMCP_WIDGET_DIR` env-var contract
- [x] 79-03-PLAN.md — Wave 3: post-deploy verifier (subprocess-spawn `cargo pmcp test {check,conformance,apps}` via `current_exe()`) + 4 verify-half flags + WARN-at-deploy-START for `OnFailure::Rollback`
- [x] 79-04-PLAN.md — Wave 4: doctor `check_widget_rerun_if_changed` + `cargo pmcp app new` build.rs scaffold + runnable example + fuzz target + `cargo-pmcp 0.12.0` version bump + CHANGELOG

---

#### Phase 78 — Gap closure (Plans 05–08, added 2026-05-02)

After cost-coach team UAT against prod (`https://cost-coach.us-west.pmcp.run/mcp`, 8 widgets, 97 tests, 33 failures — all confirmed false positives), 5 gaps were filed in 78-VERIFICATION.md and 4 gap-closure plans were spawned. AC-78-1, AC-78-2, AC-78-3 fail at the binary boundary against real prod; library-boundary verification (9/9 truths) was already passing. The cost-coach prod evidence: bundled widgets contain mangled constructor identifiers (e.g. `new yl({name:"cost-coach-cost-summary",version:"1.0.0"})`) that defeat the v1 `new App\(` regex, the `[ext-apps]` package name only survives as a log-prefix string (not the import literal `@modelcontextprotocol/ext-apps`), and the v1 SDK-detection failure cascades to all 8 handler/connect checks, producing `1 false negative → 8× false negatives` per affected widget.

**Plans (all `gap_closure: true`):**

**Wave 1**

- [x] 78-05-PLAN.md — RED-phase regression fixtures: 3 bundled HTML fixtures + `app_validator_widgets_bundled.rs` integration tests asserting verdict shape per fixture × mode; tests MUST FAIL today (G5)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 78-06-PLAN.md — Validator core fixes (G1+G2+G3): minification-resistant SDK-presence signals (`[ext-apps]` log prefix + `ui/initialize` + `ui/notifications/tool-result` method literals); mangled-id-tolerant constructor regex; eliminate SDK-to-handler/connect cascade

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 78-07-PLAN.md — `cargo pmcp test apps --widgets-dir <path>` source-scan flag (G4): scan `<path>/*.html` instead of fetching via `resources/read`; mirrors `cargo pmcp preview --widgets-dir` semantics; 3 CLI-boundary integration tests via `assert_cmd`

**Wave 4** *(blocked on Wave 3 completion)*

- [ ] 78-08-PLAN.md — ALWAYS coverage extension + docs + HUMAN-UAT re-bind: new `prop_g3_handler_detection_independent_of_sdk` proptest, `validate_widget_pair` example demos cost-coach prod-bundle shape, READMEs document `--widgets-dir`, `78-HUMAN-UAT.md` rewritten with 6 re-bound items including cost-coach prod re-verify (Test 6)

**Re-verification gate:** After Plan 06 lands, the cost-coach v1 run (97 tests, 33 false positives) must be re-executed and report zero false-positive failures on the 8 prod widgets. The 5 deferred AC-78-1..5 items are re-bound to the post-Plan-07 `--widgets-dir` path so binary-boundary verification no longer requires the deferred fixture binary `mcp_widget_server.rs.todo`.

#### Phase 78 — Gap closure cycle 2 (Plans 09-11, added 2026-05-02)

After cycle-1 closure (Plans 05-08 completed 2026-05-02), the operator re-ran Test 6 against `https://cost-coach.us-west.pmcp.run/mcp` and got the SAME 33 Failed rows. The cycle-1 synthetic fixtures didn't generalize to real Vite-singlefile prod output. Per-widget breakdown in `uat-evidence/2026-05-02-cost-coach-prod-rerun.md`: G2 constructor regex misses 8/8 prod widgets; G1 SDK signals miss 4/8. Diagnosis: Plan 05's fixtures were modeled from feedback-described shape, not bytes captured from prod — RED→GREEN passed against the model, missed reality. Cycle 2 binds the regression set to bytes captured from real prod.

**Plans (all `gap_closure: true`):**

**Wave 1**

- [ ] 78-09-PLAN.md — Real-prod fixture capture (RED phase): 6 cost-coach prod widget bundles fetched from live cost-coach prod (or local checkout) into `tests/fixtures/widgets/bundled/real-prod/` + CAPTURE.md provenance + 7 RED-phase integration tests (6 real-prod fixtures × claude-desktop + 1 cycle-1 no-regression sentinel) bound to those bytes; tests MUST FAIL today (G6)

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 78-10-PLAN.md — Validator G1+G2 generalization (GREEN phase): derive new SDK-presence + constructor patterns from real-prod CAPTURE.md grep evidence; widen mangled-id cap, add quoted-key tolerance + reordered-key support to G2; OR new G1 signals into has_sdk; preserve cycle-1 unit/property/integration tests; PMAT cog ≤ 25 + zero SATD; new G2-false-positive-guard property test guards against the widening risk

**Wave 3** *(blocked on Wave 2 completion)*

- [ ] 78-11-PLAN.md — ALWAYS-coverage extension + HUMAN-UAT cycle-2 rewrite + Test 6 re-verification checkpoint: extend `validate_widget_pair.rs` example with 6 cycle-2 real-prod widget runs + tally + success-path summary; rewrite `78-HUMAN-UAT.md` with cycle-2-explicit Test 6 acceptance bar (zero Failed rows on 8 cost-coach prod widgets); operator re-runs Test 6 against prod and resumes with `approved` (flips `gap_closure_validated: false → true`, routes to `/gsd-verify-work`) or `failed: <reason>` (routes to `/gsd-plan-phase 78 --gaps` for cycle 3)

**Re-verification gate (cycle 2):** Plan 11 Task 3 is the load-bearing gate. Operator runs `cargo pmcp test apps --mode claude-desktop https://cost-coach.us-west.pmcp.run/mcp` against real prod and confirms zero Failed rows on the 8 production widgets. On pass: phase 78 closes via `/gsd-verify-work`. On fail: phase 78 routes to a third gap-closure cycle with diagnosis in a new `uat-evidence/<date>-cost-coach-prod-cycle3-rerun.md` evidence file.

### Phase 81: Update pmcp-book and pmcp-course with v2 advanced topics (code-mode, tasks, skills)

**Goal:** [To be planned]
**Requirements**: TBD
**Depends on:** Phase 80
**Plans:** 10/10 plans complete

Plans:

- [x] TBD (run /gsd-plan-phase 81 to break down) (completed 2026-05-15)

**Cross-cutting constraints:**

- Every behavioral-prose claim about Tasks (SSE, serverless, owner binding, experimental.tasks, TaskSupport::*, tasks/result, tasks/cancel, tasks/get, poll interval, pollInterval, CreateTaskResult) still accurately describes current `pmcp-tasks` behavior (revision R-5 — prose drift, not just type-name drift).

### Phase 105: Task poll-decision classifier and durable-consumer docs

**Goal**: Make `TaskStatus::InputRequired` an actionable state for every consumer shape by factoring the terminal/pollable/input-required poll decision OUT of `Client::wait_for_task`'s loop into a shared, loop-free classifier — `Terminal { status } | InProgress { poll_hint } | InputRequired` (3 variants, unit `InputRequired`, no `Unpollable` — revised per CONTEXT.md D-03/D-05 after Codex review) — consumed internally by `wait_for_task` (D-05 single-decision discipline: the two poller shapes cannot drift) and callable per-poll by durable/replay consumers (Temporal-style `ctx.step`/`ctx.wait` loops that cannot block). Plus the docs half: a "Consuming tasks from a durable/replay workflow" page (rustdoc next to `wait_for_task` + pmcp-book) covering the typed-accessors-without-the-loop pattern, the replay-determinism caveat, and an explicit "when NOT to use `wait_for_task`" section. Note the classifier is a pure function of the polled `Task` — the terminal `CallToolResult` still comes from a separate `tasks/result` call the consumer owns (e.g., as its own memoized durable step).

**Scope fences (LOCKED):** no wire changes (`tasks/provide_input` explicitly REJECTED as spec-invention — polling-client input provision is an upstream spec gap; the classifier's `InputRequired` variant is the seam for when the WG standardizes it); no new `TaskStatus` variants; no change to `wait_for_task` blocking behavior or its `input_required` typed-error default (2.12.0 CR-01 fix stays).

**Origin:** pmcp.run dev-team request `~/Development/mcp/sdk/pmcp-run/.planning/notes/sdk-issue-durable-task-consumer-and-input-required.md` — Ask A accepted (this phase), Ask B (task elicitation round-trip) deferred as spec-shaped, Ask C answered; SDK response at `pmcp-run/.planning/notes/sdk-response-durable-task-consumer-and-input-required.md`.

**Requirements**: D-01..D-16 (CONTEXT.md decision set; no separate REQ IDs mapped)
**Depends on:** Phase 104 (shipped in pmcp 2.12.0)
**Plans:** 3/3 plans complete

Plans:

**Wave 1**

- [x] 105-01-PLAN.md — Pure poll-decision primitive: `TaskPollDecision` enum + `Task::poll_decision()` + `resolve_poll_interval()` in `src/types/tasks.rs` (Wave 1)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 105-02-PLAN.md — Rewrite `wait_for_task` as `match task.poll_decision()` + drift-pin regression test (Wave 2)
- [x] 105-03-PLAN.md — Runnable s48 example + "Durable and replay consumers" book section (Wave 2)

---

### Phase 80: SEP-2640 Skills Support

- [x] **Phase 80: SEP-2640 Skills Support** — Add the experimental Skills extension (SEP-2640) as a batteries-included PMCP feature. Includes (a) a one-line additive change to `ServerCapabilities` adding an `extensions` field parallel to `experimental`, (b) a new `Skill` / `SkillReference` / `Skills` DX layer behind a `skills` feature flag built as sugar over the existing `ResourceHandler` trait, (c) builder methods `.skill(...)` / `.skills(...)` / `.bootstrap_skill_and_prompt(...)` on `ServerCoreBuilder` with internal composition over any pre-existing `.resources(...)` handler, (d) the dual-surface pattern — same skill data exposed via both a SEP-2640 skill surface AND a parallel MCP prompt surface (for hosts that don't yet support SEP-2640) — with byte-equal-by-construction invariant, (e) paired `examples/s38_server_skills.rs` + `examples/c38_client_skills.rs` demonstrating three tiers of skills (hello-world, refunds, code-mode), and (f) integration test asserting both surfaces produce byte-equal content. (completed 2026-05-13)

**Goal:** A PMCP server author can register an Agent Skill in ~5 lines of code, and the same skill content is automatically reachable via two parallel surfaces: SEP-2640 skill resources (for capable hosts) and an MCP prompt (for everyone else). The two surfaces are derived from a single `Skill` value so they cannot drift.

**Depends on:** None (no protocol breaking change; additive `extensions` field is backward-compatible).

**Source of truth:** Spike findings packaged at `.claude/skills/spike-findings-rust-mcp-sdk/` (spikes 001 + 002 both VALIDATED). Reference implementation lives at `.planning/spikes/002-skill-ergonomics-pragmatic/src/main.rs` — the `Skill`, `SkillReference`, `Skills`, `SkillsHandler`, and `ComposedResources` types lift near-verbatim into the real implementation.

**Out of scope (deferred to v2):**

- SEP-2640 §4 archive distribution (`application/gzip` + base64 blob). Blocked by GAP #2 (`Content::Resource` has no `blob` field). The SEP marks archive mode as optional.
- `#[pmcp::skill]` procedural macro for compile-time SKILL.md validation. Worth a separate spike if compile-time validation is wanted.
