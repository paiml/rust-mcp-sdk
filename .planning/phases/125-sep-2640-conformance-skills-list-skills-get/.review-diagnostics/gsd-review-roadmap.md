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

- [ ] Phase 125: SEP-2640 Conformance — skills/list + skills/get

### Phase 125: SEP-2640 Conformance — skills/list + skills/get

**Goal**: A pmcp server that declares `io.modelcontextprotocol/skills` actually answers it. The current SEP-2640 draft makes `skills/list` and `skills/get` mandatory for any server declaring the extension; the shipped module auto-declares and implements neither, so a conforming host's first `skills/list` call gets -32601. This phase routes both methods via the crate-private `InternalClientRequest` classifier (pattern at `src/types/protocol/mod.rs:583` — NO new public `ClientRequest` variant, 2.x promise), answers them entirely from the shipped `Skills` registry with conforming entries (verbatim frontmatter JSON + complete `{uri, digest: sha256, size}` manifests), retires (or legacy-gates) the nonstandard `skill://index.json`, validates name-identity (final URI segment == frontmatter `name`) at build, and guards the ≤512-file / ≤16 MiB limits at `into_handler()`.
**Depends on**: Nothing in-repo (first phase of the milestone). Spike 008
(`.planning/spikes/008-sep-2640-drift-check/`) is the measured drift evidence; the fix contract
lives in `.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md`.
**Requirements**: TBD — no formal REQ-IDs; the tracked requirement set is `125-CONTEXT.md`
decisions D-01..D-11 (all 11 covered by the plans below; gate `check.decision-coverage-plan`
reports 11/11).
**Plans:** 5 plans across 4 waves — 1:{01} 2:{02,03} 3:{04} 4:{05}

Plans:
**Wave 1**

- [ ] 125-01-PLAN.md — TRACER: `skills/list` end-to-end over streamable HTTP + the SC#1 routing guarantees

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 125-02-PLAN.md — `skills/get` with draft-correct -32602 semantics + `ServerCore` twin-site parity
- [ ] 125-03-PLAN.md — Complete `resources` manifests, verbatim frontmatter, D-02 warn+exclude, name-identity reject, SEP limits warning

**Wave 3** *(blocked on Wave 2 completion)*

- [ ] 125-04-PLAN.md — Retire `skill://index.json` (12 tracked sites) + examples and the four documentation surfaces

**Wave 4** *(blocked on Wave 3 completion)*

- [ ] 125-05-PLAN.md — `make test-skills` gate leg with a zero-test-count guard, fuzz target, and the rustdoc deferral record

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
