# Phase 119: Documentation — Three Shapes + v2 Migration - Context

**Gathered:** 2026-08-18
**Status:** Ready for planning

<domain>
## Phase Boundary

The v2.5 milestone is documented per the house three-shapes rule — **pmcp-book chapter +
runnable example + README/course** — leading with the `cargo pmcp` workflow, covering both the
v2.4 Agents & Teams surface (carried from Phase 111) and the v2 dual-version migration story,
with runnable v2 examples verified against the shipped code.

Three requirements:

- **DOCS-04** — Agents & Teams documented in three shapes, cargo-pmcp-first
- **DOCS-05** — v2 migration guide + dual-version documentation: how to opt into v2, the
  dual-version story, Tasks extension migration, and the legacy sunset policy
- **DOCS-06** — Runnable v2 examples: a stateless (Lambda-style) v2 server and a v2
  client/agent example

**This is a documentation phase with two deliberate, bounded code exceptions**, both taken by
explicit decision below: the Phase-113 arm-1 record (D-01) and the example-verification
harness (D-13/D-14). It writes no new example code (D-12) and no new SDK feature.

**Not in scope:** filling the book's 20 other "Coming Soon" stub chapters; enabling mdbook
doctests; renumbering examples; UNAS-01 (SEP-2243 `x-mcp-header`, carried to v2.6).

</domain>

<decisions>
## Implementation Decisions

### Relationship to Phase 113's undischarged hold

- **D-01: Phase 119's first plan is "task zero"** — it formally runs and records Phase 113's
  arm-1 re-verification, upgrades `113-SPEC-RECHECK.md`'s `## Verdict`, and flips HTTP-01..08
  and CLNT-01/02/05 from `[~]` to `[x]`. Docs are then written against a settled, recorded
  surface with no `[~]` hedging anywhere.

  **The measurement is already done — it was taken during this discussion (2026-08-18) and
  came back clean. The planner must RE-RUN it as the plan's own evidence, not cite this
  block.** What was measured:

  | Check | Result |
  |---|---|
  | Newest versioned schema dir upstream | `2026-07-28` (only `draft` is newer; draft is not a release) |
  | Last commit touching `schema/2026-07-28` | still the pinned `271ecc9accafdd9b83a3c869fa67c22953b2af80`, 2026-07-28T16:42:34Z — **nothing in 21 days** |
  | `schema.ts` blob vs upstream `main` | `9b55feeb412bc3ae877f2eac10b5c01ba29a2eed` — **identical** |
  | `schema.json` blob vs upstream `main` | `213c58f6d9a1c2ce6ad055afe90bbdb095a29ee8` — **identical** |
  | Arm 1 step 2 — the three identifiers | all present, 2 hits each |
  | Arm 1 step 3 — code mappings | `HEADER_MISMATCH = -32020` (`schema.ts:434`), `MISSING_REQUIRED_CLIENT_CAPABILITY = -32021` (`:442`), `UNSUPPORTED_PROTOCOL_VERSION = -32022` (`:450`) |
  | Arm 1 step 3 — HTTP-400 mappings | all three interfaces state "MUST be `400 Bad Request`" |
  | Arm 1 step 3 — payload shapes | `supported: string[]`, `requested: string`, `requiredCapabilities: ClientCapabilities` |

  **Landing state: `PUBLISHED-CONFIRMED`.** Arm 2 already ran and recorded NO DRIFT (plan
  113.1-04, § B.6.5), so both arms are satisfied and the obligation's shared landing state
  (arm 1 step 4) is reachable.

  **The `## Verdict` was NOT upgraded during discussion and no requirement was flipped** — a
  formal verdict change is not a side effect a discuss-phase may have. That is task zero's job.
  Per arm 1 step 4, requirements may be flipped only once BOTH arms are run and recorded; a
  `## Verdict` upgraded on arm 1 alone is invalid.
  — **Reversibility:** one-way — flipping eleven requirements and upgrading a formal verdict
  changes the project's requirements ledger and discharges a recorded obligation traceable to a
  named person and date; unflipping would require reopening Phase 113.

- **D-02: The migration chapter owns a "Behaviour changes & known limitations" section**
  consolidating the CHANGELOG `[2.19.0] - Unreleased` wire changes, the four consumer-observable
  disclosure entries in `.planning/WINDOWS.md` (12, 13, 19, 20) and CR-03 (entry 23).
  `CHANGELOG.md` stays the **per-release** record; the guide is the **per-migration** one. The
  two-places-to-sync cost is accepted deliberately, and D-03 pays for it.

- **D-03: A tripwire test keeps that section from rotting.** The guide cites each disclosure by
  its `.planning/WINDOWS.md` entry id, and a test asserts every row flagged consumer-observable
  has a matching citation in the guide. A new disclosure that skips the guide fails CI. Chosen
  over greppable-ids-and-review-discipline because the milestone's habit is tripwires over
  convention (`full` vs `full-v2` drift, vendored-schema digests).

  **Planner must handle:** `.planning/WINDOWS.md` rows currently carry `kind: deviation` with no
  consumer-observable marker. The tripwire needs something to key on — either a new `kind`
  value, a flag column, or a convention the test can match. Decide this before writing the test.

### Migration guide home and naming

- **D-04: The v2 migration guide is a NEW pmcp-book chapter in Part III**, alongside 12.10–12.14
  (the other cargo-pmcp-first feature chapters). It holds the opt-in path, the dual-version
  story, the Tasks migration pointer, the D-02 limitations section, and links to
  `docs/v1-sunset-policy.md` (already normative, written in Phase 117 — do NOT restate it).

  Two existing docs are deliberately **left untouched**, because each already owns a different
  sense of "migration":
  - `docs/MIGRATION.md` (597 lines, 16 sections) — pmcp crate **1.x → 2.0** (import paths,
    removed aliases, `#[non_exhaustive]` breakage)
  - `pmcp-book/src/ch21-migration.md` — an 11-line "Coming Soon" stub in *Part VI: TypeScript
    SDK Compatibility*, i.e. **TypeScript → Rust**

- **D-05: Naming convention** — protocol eras keep `v1`/`v2`; the crate is ALWAYS written with its
  name attached ("pmcp 2.19"), never bare "v2.0". Era naming matches the entrenched code
  vocabulary a reader will meet (`v1-compat`, `full-v2`, `v1_session.rs`, `s47_v2_*`,
  `docs/v1-sunset-policy.md`). Each new doc opens with a one-line disambiguation callout.
  Rejected: always-dated eras ("MCP 2026-07-28") — unambiguous in prose but diverges from the
  feature flags and file names.

- **D-06: The chapter's organizing spine is BY ROLE — server / client / agent** — because
  "how to opt into v2" has a genuinely different answer per role:

  | Role | What changes |
  |---|---|
  | **Server** | Nothing to opt into — one binary serves both eras by per-request negotiation. The only lever is opting *out* of v1: `cargo build --no-default-features --features full-v2` |
  | **Client** | Explicit: `ClientBuilder::with_protocol_version(PROTOCOL_VERSION_2026_07_28)` (`src/client/mod.rs:5190`). Explain why there is **no auto-probe** — Phase 113 D-08 lock, carried in-source at `src/client/mod.rs:871-878` |
  | **Agent** | `pmcp-agent` prefers v2 with v1 fallback; the era probe lives in `pmcp-agent`, NOT in `Client` (117 A-D08) |

  Each track leads with the `cargo pmcp` workflow. A reader finds their track and stops.

- **D-07: The Tasks era-delta AMENDS the existing `pmcp-book/src/ch12-7-tasks.md`**
  (extension-ization, `tasks/list` removed on v2, capability-negotiation change), and the
  migration chapter's role tracks link to it. That chapter is 799 lines and mentions
  `2026-07-28` **zero times** — it currently reads as if v2 does not exist. Fixing it where
  Tasks readers already are beats writing a parallel account.
  — **Reversibility:** costly — edits a large, load-bearing existing chapter rather than adding
  a new file.

### Agents & Teams (DOCS-04)

- **D-08: Two NEW Part III chapters — "Agents as MCP Clients" and "Agent Teams"** — beside the
  new migration chapter and 12.10–12.14. Plus **re-parent the existing
  `pmcp-book/src/ch17-04-sampling-hosting.md`** so it is reachable.

  That third chapter **already exists and is written** — 103 lines carrying exactly the
  LLM-server disambiguation Phase 111 asked for ("Spec host sampling (server asks the client)" /
  "LLM-server pattern (client asks a server)" / "Contrasting the two directions"). It is
  currently parented under `ch17-examples.md`, which is itself a "Coming Soon" stub, so it is
  effectively unreachable. Phase 111's three named chapters are therefore **two to write, one
  to relink**.

- **D-09: pmcp-course gets ONE new Part VIII chapter + exercises for Agents & Teams**, matching
  the `ch23-skills.md` depth (443 lines + 222 lines of exercises: Learning Objectives, Tier
  1/2/3 hands-on progression, cross-SDK compatibility). **The v2 migration gets NO course
  chapter** — book-and-README only. This tracks the requirement wording literally: DOCS-04 names
  "README/course"; DOCS-05 does not mention the course.

- **D-10: Docs cite examples by their FULL runnable cargo invocation, never a bare number.**
  No renumbering, no new index page. e.g.
  `cargo run -p pmcp-agent --example s50_standalone_vs_sampled`.

  The numbered namespace has collided — `s49` appears twice in root `examples/`
  (`s49_sampling_host.rs`, `s49_v2_subscriptions_client.rs`) and `s50` appears in both root
  (`s50_v2_tasks_server.rs`) and `pmcp-agent` (`s50_standalone_vs_sampled.rs`). Renumbering was
  rejected because completed phase SUMMARYs, plans and CHANGELOG entries name these files, and
  the milestone has been careful about that traceability. Full invocations are unambiguous
  anyway, since cargo needs `-p` to reach the non-root ones.

- **D-11: README gets two new sections, cargo-pmcp-first** — `## Agents & Teams` (the
  `cargo pmcp agent new` / `team dev` workflow first, then the crates) and `## Protocol
  Versions` (dual-version story, linking the new migration chapter). Plus runnable commands
  added to the `## Examples` block, and a refresh of the stale `## Latest Release: v2.0.0`
  header at `README.md:447` (CHANGELOG shows `2.19.0 - Unreleased`; 2.17.0 is published).
  Today Agents & Teams has exactly one bullet, at `README.md:24`, and eras have nothing.

### Runnable v2 examples (DOCS-06)

- **D-12: The existing examples satisfy DOCS-06's runnable requirement as-is** — the phase writes
  NO new example code. "Lambda-style" is read as the **stateless architecture**, which
  `examples/s47_v2_stateless_mrtr.rs` already demonstrates (no `initialize`, no
  `Mcp-Session-Id`, AEAD-sealed `requestState`, resumption across an independent HTTP request).
  What is missing is **prose**, and that is what the phase adds:

  - The **Lambda deployment story** — `crates/pmcp-server/pmcp-server-lambda/src/main.rs` (272
    lines) is billed as "the standard pattern for running any pmcp server on AWS Lambda" and
    mentions `2026-07-28` **zero times**; nothing connects Lambda to v2 statelessness.
  - **`PMCP_REQUEST_STATE_KEY`** — called "a SECURITY-RELEVANT deployment decision" in s47's own
    header, yet it appears in **zero markdown files** repo-wide. Promote it out of the doc
    comment into the migration chapter's **server** track.

  Rejected: writing a new Lambda-deployable example (new code in a docs phase, plus a deploy
  story to keep working), and making the lambda crate era-aware (touches a shipped crate).

- **D-13: "Verified against the shipped code" gets a real mechanism** — spawn-and-assert run tests
  for the cited examples, PLUS a repair of `make test-examples`.

  Neither existing mechanism verifies anything today:
  - `Makefile:255` `test-examples` states "Examples are built but **not run** to avoid blocking
    on I/O", and swallows build failure into `⚠ Example $$example requires specific features
    (skipped)` while still exiting 0. An example that stops compiling reports as *skipped*, not
    *failed*.
  - `pmcp-book/book.toml:77` disables doctests entirely, so no book code block is compile-checked.

  The house RUN precedent to follow: `tests/embedded_resource_example_run.rs` and
  `tests/log_records_example_run.rs` spawn built binaries and assert on real output.

  **Book doctests stay OUT of scope** (see `<deferred>`).
  — **Reversibility:** costly — turning a lenient repo-wide target strict is felt by every
  contributor, and reverting it re-opens the false green.

- **D-14: Pre-existing breakage is BASELINED BEFORE the gate change**, so it is never attributed
  to this phase; what is genuinely cheap gets fixed; the remainder is logged to
  `deferred-items.md` with measured error counts. This is exactly the 118.1-03 precedent (the
  four workspace-excluded example crates were measured at base first, three fixed,
  `examples/26-server-tester`'s 8 residual errors logged). The gate lands green against a
  **recorded baseline**, not against zero.

- **D-15: The cited/gated set is SIX examples across three crates**:

  | Example | Crate | Requirement |
  |---|---|---|
  | `s47_v2_stateless_mrtr` | root | DOCS-06 (stateless v2 server) |
  | `s48_v2_mrtr_client` | root | DOCS-06 — s47's pair; s47 demonstrates nothing without it |
  | `s53_v2_agent_client` | root | DOCS-06 (v2 client/agent) |
  | `s49_sampling_host` | root | DOCS-04 (sampling host) |
  | `s50_standalone_vs_sampled` | `pmcp-agent` | DOCS-04 (standalone vs hosted, same loop two sources) |
  | `doc_review_team` | `pmcp-team-servers` (needs `runtime` feature) | DOCS-04 (small team end-to-end) |

  Requirement-named, plus the one pairing that makes a named example usable.
  `s54_v2_dual_conformance` was considered and left out — it already runs under the conformance
  suite and widening the gated set beyond what DOCS-04/06 name was declined.

### The ch10 transport chapters (resolved after research, 2026-08-18)

- **D-16: The two stale `ch10` transport chapters are scoped IN, minimal-touch** (research option
  B) — as a single plan task, with an explicit acceptance criterion that NO code block in either
  chapter is modified. This settles the `<deferred>` item below; the researcher measured the
  surface and the user chose option B over leaving it out (A) or rewriting the taxonomy (C).

  What the task does, and nothing more:

  1. Make the two protocol-version lines era-aware — `pmcp-book/src/ch10-transports.md:572` and
     `pmcp-book/src/ch10-03-streamable-http.md:73`, both under a "Headers enforced by the server"
     bullet, both currently naming only `2025-11-25`. Two-token edits.
  2. Add ONE `> **Era note**` callout at the head of `## Understanding Streamable HTTP Modes`
     (`ch10-transports.md:184-273`) stating that the three modes documented there are **v1
     build-time** modes and that v2 statelessness is **per-request and orthogonal**, linking the
     new migration chapter's server track (D-06). **The callout must ALSO cover SSE resumption**
     — see the correction below.
  3. Add the D-05 one-line era-disambiguation callout at the top of each of the two chapters.

  **CORRECTION to research § F-1 (found by the pattern mapper, verified 2026-08-18).** F-1
  reported that `Last-Event-ID` appears **zero** times in either chapter and concluded "the
  exposure is omission, not contradiction." That grep used the wrong casing. The real spelling is
  `Last-Event-Id`, and it appears **three** times:

  | Location | Text |
  |---|---|
  | `ch10-transports.md:257` | `- Supports resumption via \`Last-Event-Id\` header` — **inside Mode 3 of the very taxonomy D-16.2 targets** |
  | `ch10-transports.md:577` | `- **\`Last-Event-Id\`**: For SSE resumption after reconnection` |
  | `ch10-03-streamable-http.md:76` | `- \`Last-Event-Id\`: For SSE resumption` |

  So the chapters DO make resumption claims, and those claims are v1-only. The `> **Era note**`
  in step 2 must therefore state that **SSE resumption via `Last-Event-Id` is a v1 mechanism**
  alongside the sessions point — otherwise the callout leaves a live contradiction in the same
  section it is there to fix. Option B still holds: this is additional callout prose, not a
  code-block edit and not a rewrite.

  **Why this and not the alternatives.** The measured protocol-version staleness is one line per
  chapter. The real defect is that
  `## Understanding Streamable HTTP Modes` teaches statelessness as a build-time config choice
  (`session_id_generator: None`, `ch10-transports.md:188-206`), which
  `examples/s47_v2_stateless_mrtr.rs:19-22` falsifies for v2 in its own header — a reader chasing
  v2 statelessness would disable v1 sessions unnecessarily. That is behavioural, not cosmetic, and
  it contradicts the very example this phase cites as DOCS-06's proof. Option C (a ~250-line
  four-mode rewrite) was rejected: it duplicates D-06's server track and re-opens three further
  sections, turning a docs phase into a chapter rewrite.
  — **Reversibility:** reversible — additive prose in two existing chapters, no code blocks
  touched, no structural or ledger change.

### Claude's Discretion

None — every area presented was decided explicitly.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### The Phase-113 hold that task zero discharges (D-01)
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md` —
  `#### Arm 1 — Schema` (the 4-step procedure and its three landing states),
  `## Recorded Exception` → `### Re-verification obligation (binding)`, `## Third Outcome
  Policy`, and `## Verdict` (currently `PENDING`). **Arm 1 step 4's three-branch table is the
  authority on what task zero may and may not flip.**
- `schema/vendored/core-2026-07-28/PROVENANCE.md` — the pin (`271ecc9accafdd9b83a3c869fa67c22953b2af80`),
  the digests, and the § "re-verify the pin" command block
- `schema/vendored/core-2026-07-28/schema.ts` §§ 434, 442, 450, 452–530 — the three error-code
  constants and the `HeaderMismatchError` / `UnsupportedProtocolVersionError` /
  `MissingRequiredClientCapabilityError` payload shapes arm 1 asserts on
- `.planning/ROADMAP.md` — the `⚠ Phase 113's Complete counts PLANS, not REQUIREMENTS` block
  immediately after the v2.5 progress table

### The v2 / dual-version story (DOCS-05)
- `docs/v1-sunset-policy.md` — **already normative**, written in Phase 117, cross-read 26/26
  claims. The new chapter LINKS to it and must not restate or contradict it. Note especially its
  table of the **seven items deliberately NOT severed**
- `.planning/phases/117-agents-tester-v1-severability/117-CONTEXT.md` — D-01/D-02 (`v1-compat`
  default-on, `full-v2` as the severance proof, why negative features were rejected), D-04 (the
  sunset policy is condition-gated: no date, no `#[deprecated]`, no runtime warning), D-07 and
  amendment A-D08 (agent prefers v2, probe lives in `pmcp-agent`)
- `Cargo.toml:242-269` — `default`, `full`, `full-v2` and `v1-compat` as actually declared
- `src/client/mod.rs:5190` (`with_protocol_version`), `:871-878` (the no-auto-probe lock comment)
- `CHANGELOG.md` `## [2.19.0] - Unreleased` — the wire-change disclosures D-02 consolidates
- `.planning/WINDOWS.md` — entries 12, 13, 19, 20 (consumer-observable behaviour changes) and 23
  (CR-03, deferred by developer decision 2026-08-18); the source the D-03 tripwire keys on

### Requirements and phase scope
- `.planning/REQUIREMENTS.md:937-939` — DOCS-04/05/06 verbatim
- `.planning/ROADMAP.md` § `### Phase 119` — goal, dependencies, three success criteria
- `.planning/ROADMAP.md` § `### Phase 111` — the carried-over v2.4 docs scope: the three named
  book chapters and the three named examples
- `.planning/REQUIREMENTS.md` § UNAS-01 — SEP-2243, **carried to v2.6, explicitly not this phase**

### Docs surfaces this phase writes or amends
- `pmcp-book/src/SUMMARY.md` — where the new Part III chapters slot (after 12.14) and where
  `ch17-04-sampling-hosting.md` is re-parented from
- `pmcp-book/src/ch12-7-tasks.md` — 799 lines, amended per D-07
- `pmcp-book/src/ch17-04-sampling-hosting.md` — already written; re-parent only
- `pmcp-course/src/part8-advanced/ch23-skills.md` + `ch23-exercises.md` — the depth/shape
  template the new course chapter matches (D-09)
- `README.md` — line 24 (the single Agents & Teams bullet), line 447 (`## Latest Release:
  v2.0.0`, stale), the `## Examples` block at ~545
- `docs/MIGRATION.md` and `pmcp-book/src/ch21-migration.md` — **left untouched** (D-04); listed
  so a planner does not mistake either for this phase's target

### Examples and verification (DOCS-06)
- `examples/s47_v2_stateless_mrtr.rs` — its header is the source text for the stateless-v2 and
  `PMCP_REQUEST_STATE_KEY` prose D-12 promotes
- `crates/pmcp-server/pmcp-server-lambda/src/main.rs` — the Lambda pattern; era-blind today
- `tests/embedded_resource_example_run.rs`, `tests/log_records_example_run.rs` — the
  spawn-and-assert precedent D-13 follows
- `Makefile:255` (`test-examples`, the false green D-13 repairs), `Makefile:547` (`doc-check`)
- `pmcp-book/book.toml:74-79` — the disabled `[rust]` doctest config and its SATD `TODO`
- `.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md` — project-local validated patterns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **All three of Phase 111's named examples already exist and are registered** —
  `examples/s49_sampling_host.rs` (225 lines),
  `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs` (380 lines),
  `crates/pmcp-team-servers/examples/doc_review_team.rs` (406 lines, `required-features =
  ["runtime"]`). DOCS-04's example shape is essentially satisfied; the gap is book/course/README.
- **Eight v2 examples already ship** and are feature-gated in `Cargo.toml`: `s47_v2_stateless_mrtr`,
  `s48_v2_mrtr_client`, `s49_v2_subscriptions_client`, `s50_v2_tasks_server`, `s51_v2_tasks_agent`,
  `s52_v2_caching_hints`, `s53_v2_agent_client`, `s54_v2_dual_conformance`.
- **`ch17-04-sampling-hosting.md` is written**, including the LLM-server disambiguation — one of
  Phase 111's three chapters is a relink, not a write.
- **`docs/v1-sunset-policy.md` is written and normative** — DOCS-05's sunset-policy sub-item is a
  link, not a new document.
- **`tests/embedded_resource_example_run.rs` / `tests/log_records_example_run.rs`** — a working
  spawn-a-built-binary-and-assert harness to copy for D-13.

### Established Patterns

- **Three shapes** = repo README + pmcp-book chapter + pmcp-course chapter, leading with the
  `cargo pmcp` CLI workflow. Shape A (a pure-config binary) and Shape B (a scaffold) are siblings.
- **Part III book chapters** are the home for cargo-pmcp-first feature docs (12.10 SQL, 12.11/12.12
  OpenAPI, 12.13/12.14 Workbook) — D-04 and D-08 both slot in there for consistency.
- **Course Part VIII chapters** run ~443 lines + ~222 lines of tiered exercises.
- **Tripwires over convention** — the milestone repeatedly encodes an invariant as a failing test
  rather than a documented rule (`full` vs `full-v2` drift, vendored-schema digests,
  double-wrap detection). D-03 follows this.
- **Measure the base BEFORE changing a gate** so pre-existing breakage is never attributed to the
  current phase (118.1-03). D-14 follows this.
- **`cargo pmcp` verbs already exist** for `agent` (`new`, `dev`, `run`, `sources`) and `team`
  (`dev`) at `cargo-pmcp/src/commands/agent/` and `.../team/` — the CLI-first framing has
  something real to lead with.

### Integration Points

- `pmcp-book/src/SUMMARY.md` — three new entries (migration, Agents as MCP Clients, Agent Teams)
  plus one re-parent
- `pmcp-course/src/SUMMARY.md` — one new Part VIII chapter + exercises entry
- `README.md` — two new `##` sections + Examples block + the stale release header
- `.planning/WINDOWS.md` schema — needs a consumer-observable marker for D-03's tripwire
- `Makefile:255` — the `test-examples` target D-13 repairs
- `.planning/phases/113-.../113-SPEC-RECHECK.md` — task zero's `## Verdict` upgrade
- `.planning/REQUIREMENTS.md` — task zero flips HTTP-01..08, CLNT-01/02/05

### Context the planner should not be surprised by

- **20 of the pmcp-book's chapters are "Coming Soon" stubs**, including all 7 appendices and
  ch13/17/18/19/20/21/22/24/25/26. New chapters land in that context; filling the stubs is not
  this phase.
- **Four chapters name `2025-11-25` and never `2026-07-28`**: `ch10-transports.md` (917),
  `ch12-7-tasks.md` (799, being amended per D-07), `ch10-03-streamable-http.md` (168),
  `ch17-04-sampling-hosting.md` (103). The two `ch10` ones are an **open scoping question** —
  see `<deferred>`.
- **Phase 114 is "Plans shipped — awaiting sign-off."** DOCS-05's Tasks sub-item and D-07's
  ch12.7 amendment both describe Phase 114's surface. The planner must confirm what the Tasks
  era-delta may state as shipped. **Unresolved — flag it rather than assume.**

</code_context>

<specifics>
## Specific Ideas

- The user's instinct on D-01 was to **check upstream before accepting a hedge** rather than
  document against a `[~]` status — which is what turned a "document with caveats" phase into a
  "discharge the hold, then document a settled surface" phase. Downstream agents should preserve
  that preference: **measure before hedging.**
- The Phase 111 chapter list is a real spec, not a suggestion: "Agents as MCP Clients", "Agent
  Teams", "Sampling & Hosting" (incl. the LLM-server pattern disambiguation).
- Every track and section leads with the `cargo pmcp` workflow, then drops to crate-level API.

</specifics>

<deferred>
## Deferred Ideas

- **The two stale `ch10` transport chapters** — `ch10-transports.md` (917 lines) and
  `ch10-03-streamable-http.md` (168 lines) both name `2025-11-25` and never `2026-07-28`, and
  the transport chapter is where v2 statelessness lands hardest (no sessions, no
  `Last-Event-ID`). Raised, explicitly not settled — the user chose to move on rather than scope
  it. **The researcher should scope this and propose in/out**; the same D-07 argument (fix
  staleness where readers already are) applies, but so does the docs-phase scope limit.
  — **RESOLVED 2026-08-18 after research (§ F-1): scoped IN, minimal-touch. See D-16 above.**
  No longer deferred.
- **Enabling mdbook doctests** — `pmcp-book/book.toml:77-79` disables them behind a
  `TODO: Enable when PMCP is published or configure test dependencies`. Declined in D-13 as the
  largest-scope option. Two live consequences: no book code block is compile-checked, and that
  `TODO` is SATD, which `CLAUDE.md` puts at zero tolerance. Belongs in a docs-infrastructure
  phase.
- **Renumbering the example namespace** — `s49` collides inside root `examples/` and `s50`
  collides across root and `pmcp-agent`. Declined in D-10 to protect traceability. If ever taken
  up, it needs a plan for the SUMMARYs, plans and CHANGELOG entries that name the files.
- **A generated example index page** — considered under D-10 as a way to solve the
  scattered-across-three-crates discovery problem; declined as one more page to keep current.
- **UNAS-01 / SEP-2243 `x-mcp-header` + `Mcp-Param-{Name}`** — measured and recorded as carrying
  to **v2.6**, with the suite scenario and four waiting check names named in
  `.planning/REQUIREMENTS.md`. Not this phase.
- **Making `pmcp-server-lambda` era-aware**, and writing a genuinely Lambda-deployable v2
  example — both considered under D-12 and declined in favour of documenting the contract.

</deferred>

---

*Phase: 119-Documentation — Three Shapes + v2 Migration*
*Context gathered: 2026-08-18*
