# Cross-AI Plan Review Request

You are reviewing implementation plans for a software project phase.
Provide structured feedback on plan quality, completeness, and risks.

## Project Context

# PMCP SDK Extensions

## What This Is

Extensions for the PMCP SDK: a `pmcp-tasks` crate implementing MCP Tasks (experimental spec 2025-11-25) with pluggable storage backends, and a complete MCP Apps developer experience — from `cargo pmcp app new` scaffolding through live preview with dual bridge modes to ChatGPT manifest generation and demo landing pages — enabling rich UI widgets served from MCP servers across ChatGPT, Claude, and other MCP clients.

## Core Value

Tool handlers can manage long-running operations through a durable task lifecycle (create, poll, complete) with shared variable state that persists across tool calls — giving servers memory without an LLM.

## Requirements

### Validated

- ✓ Core protocol types (Task, TaskStatus, CreateTaskResult, etc.) matching MCP spec 2025-11-25 — v1.0
- ✓ Task status state machine with validated transitions (5 states, 46 transition tests) — v1.0
- ✓ TaskStore trait with pluggable storage backends (11 async methods) — v1.0
- ✓ In-memory storage backend for dev/testing (DashMap, atomic transitions) — v1.0
- ✓ TaskContext for ergonomic handler integration (typed accessors, status transitions) — v1.0
- ✓ PMCP extension: task variables as shared client/server scratchpad via `_meta` — v1.0
- ✓ Server/client task capability types and negotiation via `experimental.tasks` — v1.0
- ✓ Tool-level task support declaration (forbidden/optional/required) — v1.0
- ✓ TaskRouter for routing tasks/get, tasks/result, tasks/list, tasks/cancel — v1.0
- ✓ Task interception for task-augmented tools/call requests — v1.0
- ✓ Owner binding security (OAuth sub, client ID, session ID fallback) — v1.0
- ✓ TaskSecurityConfig with configurable limits (max tasks, TTL, variable size) — v1.0
- ✓ Comprehensive test suite: unit (200+), property (13), integration (11), security (19) — v1.0
- ✓ Basic tasks example (60_tasks_basic.rs) — v1.0
- ✓ Task-aware workflow prompts that create tasks and bind step progress — v1.1
- ✓ Partial server-side execution with automatic pause on unresolvable steps — v1.1
- ✓ Structured prompt reply conveying completed steps, remaining steps, and task ID — v1.1
- ✓ Step state tracking in task variables (standard schema: goal, steps, completed, remaining) — v1.1
- ✓ Client continuation pattern via direct tool calls guided by prompt reply — v1.1
- ✓ Working example demonstrating task-prompt bridge with multi-step workflow — v1.1
- ✓ Lower-level KV storage backend trait for pluggable persistence — v1.2
- ✓ GenericTaskStore that delegates to any StorageBackend implementation — v1.2
- ✓ InMemoryBackend refactored from existing InMemoryTaskStore — v1.2
- ✓ DynamoDB backend behind `dynamodb` feature flag (cloud-only tests) — v1.2
- ✓ Redis backend behind `redis` feature flag (proving the trait) — v1.2
- ✓ Automated feature-flag verification across all backend combinations — v1.2
- ✓ mcp-preview widget iframe rendering with working MCP bridge proxy — v1.3
- ✓ WASM in-browser MCP client with proxy/WASM toggle and standalone polyfill — v1.3
- ✓ Shared bridge library (App, PostMessageTransport, AppBridge) eliminating inline JS — v1.3
- ✓ File-based widget authoring with WidgetDir hot-reload and bridge auto-injection — v1.3
- ✓ `cargo pmcp app new` CLI scaffolding with documented bridge API and CSP helpers — v1.3
- ✓ ChatGPT-compatible ai-plugin.json manifest generation — v1.3
- ✓ Standalone demo landing pages with mock bridge — v1.3
- ✓ Chess, map, and dataviz MCP App examples shipping — v1.3
- ✓ 20 chromiumoxide CDP E2E browser tests across 3 widget suites — v1.3
- ✓ Book Ch 14 (Performance & Load Testing) — 961-line comprehensive chapter with CLI, config, metrics, CI/CD — v1.4
- ✓ Book Ch 15 Load Testing cross-reference section — v1.4
- ✓ Book Ch 12.5 (MCP Apps) rewritten with WidgetDir, cargo pmcp app, adapter pattern — v1.4
- ✓ Course Ch 18-03 hands-on load testing tutorial (952 lines) — v1.4
- ✓ Course Ch 12 Load Testing cross-reference section — v1.4
- ✓ Course Ch 20 sub-chapters rewritten with WidgetDir/mcpBridge/adapter paradigm — v1.4
- ✓ Course quizzes and exercises for load testing and MCP Apps content — v1.4
- ✓ Examples cleanup: 17 orphans registered, 63 files role-prefixed (s/c/t/m), accurate PMCP README index, protocol badge 2025-11-25 — v2.1 (Phase 65)
- ✓ Macros documentation rewrite: deleted deprecated `#[tool]`/`#[tool_router]`/stub `#[prompt]`/`#[resource]` from pmcp-macros (898 LOC, 46% of crate); rewrote pmcp-macros/README.md from scratch (355 lines, 5 compiling `rust,no_run` doctests for all four `mcp_*` macros) wired via `#![doc = include_str!("../README.md")]`; published pmcp-macros 0.5.0 and pmcp 2.3.0 with full MACR-02 migration guide — v2.1 (Phase 66)
- ✓ Client host surface: server→client `sampling/createMessage` (incl. tools/tool_choice), `elicitation/create`, `roots/list` via builder-registered handler registry; preflight sampling approval; registry-authoritative capability derivation — v2.4 (Phase 106, pmcp 2.16.0)
- ✓ `pmcp-package` adopted + wire-frozen (pinned-digest golden fixtures for all four kinds); team-server tool contracts as provable-contracts YAML + conformance fixtures — v2.4 (Phase 107)
- ✓ `pmcp-agent` 0.x loop crate: pure loop between effect seams (`CompletionSource`/`ToolInvoker`/`ConversationStore`), `SamplingSource` + feature-gated `OpenAiCompatSource`/`AnthropicSource`, agent-as-server adapter, tasks-aware `poll_decision` — v2.4 (Phase 108)
- ✓ `pmcp-team-servers` reference crate: team-fs, mem-mcp (BM25), approval-mcp, team-mcp (members as agent-as-server tools), additive pmcp-core namespaced `_meta`, in-process `TeamRuntime`, exportable wire-level conformance harness — v2.4 (Phase 109)
- ✓ cargo-pmcp 0.18.0 agent/team verbs: `agent new`/`agent dev`, `team dev`, `package inspect|capture` with version-pin tripwires — v2.4 (Phase 110)
- ✓ All 7 v2.4 crates published to crates.io as pmcp v2.17.0 (2026-07-19, PR #302/tag v2.17.0)
- ✓ docs.rs pipeline and feature flags: `Cargo.toml` `[package.metadata.docs.rs]` replaced `all-features = true` with explicit 15-feature list + dual targets (`x86_64-unknown-linux-gnu` + `aarch64-unknown-linux-gnu` for first-class ARM64/Graviton coverage); created crate-focused `CRATE-README.md` at repo root (171 lines, 18-row Cargo Features table) wired into `src/lib.rs` via `#![doc = include_str!("../CRATE-README.md")]` (matches Phase 66 pmcp-macros pattern, pulls DOCD-02 from Future Requirements into scope); fixed all 29 rustdoc warnings across 16 source files (+8 residual links orchestrator-applied) via the "demote to backticks" pattern; adopted `feature(doc_cfg)` (post-RFC 3631 now provides auto-cfg badging by default — original D-01 `doc_auto_cfg` flip was invalidated by Rust 1.92.0 upstream removal, amended mid-phase); added new `make doc-check` target (stable toolchain, D-16 feature list, TAB-indentation guarded) and CI `Check rustdoc zero-warnings` step inside the existing `quality-gate` job (deliberately NOT chained into local `make quality-gate` per D-27 to protect developer iteration speed); no pmcp version bump (D-28 — stays at 2.3.0, docs.rs re-renders on next unrelated release); human-verify nightly badge checkpoint APPROVED — v2.1 (Phase 67)

- ✓ Version plumbing & negotiation (VERS-01..09): per-request `_meta` clientInfo, `server/discover`, required `Mcp-Method`/`Mcp-Name`/`MCP-Protocol-Version` headers, error-code rename `-32002`→`-32602` — v2.5 (Phase 112)
- ✓ Stateless streamable-HTTP + multi-round-trip elicitation (HTTP-01..08): `initialize`/`initialized` and `Mcp-Session-Id` removal path, `InputRequiredResult`/`requestState`; era resolved from the RAW request body so every method — not just the three `_meta`-bearing ones — can be a v2 request — v2.5 (Phase 113)
- ✓ Tasks extension migration (TASK-01..06): `tasks/list` removed, `tasks/update` added, server-directed creation; the v1.x DynamoDB/Redis task-store investment survived as an API reshape only — v2.5 (Phase 114)
- ✓ JSON Schema 2020-12 + caching hints (SCHM-*): `structuredContent` as any JSON value, `ttlMs`/`cacheScope`, position-aware `$schema` traversal derived from the pinned meta-schemas rather than hand-kept keyword lists — v2.5 (Phase 115)
- ✓ Auth-hardening SEPs (AUTH-*): RFC 9207 `iss` validation, DCR `application_type`, the six SEPs — v2.5 (Phase 116)
- ✓ v1 severability (SMPL-01/02, CLNT-03/04): default-on `v1-compat` plus a `full-v2` feature set severing v1 machinery at compile time via signature-identical paired modules, so call sites carry no `#[cfg]`; CI severance gate is a real merge blocker — v2.5 (Phase 117)
- ✓ Client & agents on v2 (CLNT-01/02/05): era-aware client, `mcp-tester --dual-run` diffing both eras against an `era-deltas.yaml` baseline — v2.5 (Phases 117, 118.2)
- ✓ Conformance against the official suite (CONF-*): nine gaps G-1..G-9 found and closed; `Content::Resource` emits the spec shape on both eras via a tolerant `#[serde(try_from)]` reader — v2.5 (Phases 118, 118.1, 118.2)
- ✓ Docs in three shapes (DOCS-*, carried from v2.4 Phase 111): pmcp-book v2 migration chapter, README/CHANGELOG protocol-era rewrite, course alignment — v2.5 (Phase 119)
- ✓ Published as pmcp v2.19.0 (2026-08-20, PR #337/tag v2.19.0)

### Active

<!-- Milestone v2.6 AI-Package Portability. Full text with traceability in .planning/REQUIREMENTS.md. -->

## Phase 125: SEP-2640 Conformance — skills/list + skills/get
### Roadmap Section

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

### Requirements Addressed

No formal REQ-IDs for this phase; the tracked requirement set is 125-CONTEXT.md (below).

### User Decisions (CONTEXT.md)

# Phase 125: SEP-2640 Conformance — skills/list + skills/get - Context

**Gathered:** 2026-09-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 125 makes the shipped `skills` module (Phase 80, feature `skills`,
`src/server/skills.rs`) honest about the capability it declares. The current
SEP-2640 draft (PR #2640 head `sep/skills-extension`, last push 2026-08-29)
makes `skills/list` and `skills/get` mandatory for any server declaring
`io.modelcontextprotocol/skills`; the shipped module auto-declares and
implements neither. This phase routes both methods via the crate-private
`InternalClientRequest` classifier (NO new public `ClientRequest` variant —
2.x exhaustive-enum promise), answers them from the shipped `Skills`
registry with conforming entries (verbatim frontmatter JSON + complete
`{uri, digest: sha256, size}` manifests), retires the nonstandard
`skill://index.json`, validates name-identity at build, and guards the
≤512-file / ≤16 MiB limits.

**In scope:** the two RPC methods over streamable HTTP; `Skills::entries()`
manifest synthesis; frontmatter parse (serde_yaml, isolated); index.json
retirement; build-time name-identity validation + limits guard; warn+exclude
semantics for frontmatter-less skills; frontmatter cleanup of canonical
surfaces (examples s44/c10, realistic integration-test skills, book
snippets); a dedicated `make test-skills` leg wired into quality-gate;
`skills/get` unknown URI → `-32602` per draft.

**Out of scope (all explicitly recorded, never silently dropped):** stdio
transport reach (D-01 deferral); `resources/directory/read` (current `{}`
declaration legitimately means `directoryRead: false`); client wrappers
(`list_skills()` / `get_skill()` / `read_skill_uri()`); fixing the shipped
`SkillsHandler::read` `-32601`-vs-`-32602` divergence for `resources/read`
(recorded as an observation; changing an existing error code is observable
behavior with its own test).

**Evidence base:** spike 008 (`.planning/spikes/008-sep-2640-drift-check/`),
the spike-findings skill
(`.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md`),
and 125-RESEARCH.md (HIGH confidence, line-cited; valid until 2026-09-15 —
re-run `gh pr view 2640 --json headRefName,updatedAt` before planning locks).

</domain>

<decisions>
## Implementation Decisions

### Transport reach (the keystone)

- **D-01:** **HTTP-only this phase.** `skills/list` + `skills/get` land over
  streamable HTTP via the `InternalClientRequest` classifier route
  (`classify_http_ingress` is its only production consumer per its own
  rustdoc). Stdio reach is a **recorded deferral with an owner** — widening
  `IngressRequest::Internal` into `run_transport_actor` means changing the
  actor's `(RequestId, Request)` channel type and is a bigger change than
  the skills work itself. The deferral must appear in the plan's deferred
  items and the module docs; it must NOT be a code TODO (SATD forbidden).
  Measured hazard to document: over stdio the frame fails at
  `parse_message` → `TransportError::InvalidMessage` → the server actor
  breaks the loop (`src/server/mod.rs:1463-1466`).

### Frontmatter semantics

- **D-02:** **Warn + exclude for frontmatter-less skills.** A skill whose
  body carries no YAML frontmatter is excluded from `skills/list`
  (SEP-legal: "MAY return an empty or partial listing"), still served via
  `resources/read`, and `skills/get` on it errors. A build-time warning
  names the excluded skill. No hard error this phase (a strict/`try_`
  variant may come later). Rationale: 40+ existing `Skill::new(...)` call
  sites keep compiling and behaving; the draft makes a synthesized
  `{name, description}` a guaranteed host-side rejection, so partial
  listing is the only honest default.

- **D-03:** **Cleanup scope: canonical surfaces only.** Examples s44/c10,
  integration-test skills that represent realistic skills, and book
  snippets gain real frontmatter so every user-facing surface produces
  conforming `skills/list` entries. Low-level unit/proptest fixtures stay
  frontmatter-less on purpose — they are the natural test coverage for the
  D-02 warn+exclude path.

### Dependencies

- **D-04:** **serde_yaml 0.9, optional, gated on `skills`.** Already in
  Cargo.lock as a production dep of four workspace crates, no RUSTSEC
  advisory, zero new packages. The parse is isolated behind ONE
  crate-private fn so swapping to a maintained fork later is a one-file
  change. (JSON-instead-of-YAML was considered and rejected: agentskills.io
  mandates YAML frontmatter in SKILL.md and hosts verify the `skills/list`
  entry's verbatim-JSON frontmatter field-by-field against the fetched
  file — the format is not ours to choose.)

- **D-05:** **sha2 is already a non-optional pmcp dep at 0.11.** Use it for
  the `sha256:{64 lowercase hex}` digests; note the spike's 0.10-era
  `format!("{:x}", …)` snippet is not a safe copy-paste (research pitfall).

### Wire conformance details

- **D-06:** **`skills/get` on an unknown URI returns `-32602`** per the
  current draft. The shipped `SkillsHandler::read` returns `-32601` for
  `resources/read` (`src/server/skills.rs:556-559`) — that divergence is
  recorded as a separate out-of-scope observation, not fixed here
  (`resources_read_unknown_uri_method_not_found` at
  `tests/skills_integration.rs:253` pins it).

- **D-07:** **Results carry `"resultType": "complete"`**, and `skills/list`
  carries `ttlMs`/`cacheScope` on protocol 2026-07-28+. Cacheability must
  be named at the projection call site — `request_is_cacheable` is keyed on
  public `ClientRequest` variants and its rustdoc forbids adding a row for
  a variant that cannot occur.

- **D-08:** **`skill://index.json` retires when `skills/list` lands** —
  removed by default (legacy gate only if a plan-time blast-radius check
  shows a consumer needs it; research charted the blast radius).

### CI / feature coverage

- **D-09:** **Dedicated `make test-skills` leg wired into quality-gate.**
  `skills` joins neither `default` nor `full` — the `full`/`full-v2`
  enumerated lists and `tests/v1_severability_tripwire.rs` stay untouched.
  This closes the measured hole where `make quality-gate` never compiles or
  tests the skills module (only `make build`/`test-examples` compile it,
  neither runs a test; zero mentions in workflows).

### Pagination

- **D-11:** **`skills/list` returns a single page.** All entries in one
  response, no `nextCursor` emitted (conformant: an absent cursor means the
  listing is complete; the shipped `SkillsHandler::list` already ignores its
  `_cursor`). Cursor pagination is a recorded deferral — revisit if a
  registry with hundreds of skills materializes.

### Capability declaration

- **D-10:** **Keep auto-declaring, now honestly.** With both MUST methods
  implemented (HTTP), `set_skills_capabilities` keeps declaring the
  extension; the current `{}` declaration legitimately means
  `directoryRead: false` and stays. The rustdoc on
  `set_skills_capabilities` documents the HTTP-only reach (D-01) and the
  `directoryRead: false` deferral.

</decisions>

<canonical_refs>
## Canonical References

- `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-RESEARCH.md` —
  line-cited architecture map, pitfalls, wire shapes (HIGH confidence).
- `.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md` —
  the 7-gap fix blueprint from spike 008.
- `.planning/spikes/008-sep-2640-drift-check/` — the measured drift evidence
  + wire-proof technique (a `from_value::<ClientRequest>` Err IS the routing
  proof; pair with a control method that parses).
- `src/types/protocol/mod.rs:583` — `ServerDiscoverRequest` doc comment: the
  sanctioned `InternalClientRequest` + `classify_internal_method` pattern.
- SEP-2640 current draft: PR modelcontextprotocol#2640 head branch
  `sep/skills-extension` raw markdown (NOT the docs site — it lags).
</canonical_refs>

<specifics>
## Specific Ideas

- Wire-proof tests in the phase should reuse spike 008's technique: assert
  `from_value::<ClientRequest>(json!({"method":"skills/list",...}))` still
  errs (2.x promise held) while the HTTP ingress classifier routes it.
- `Skills::entries()` computed at `into_handler()`/build time is the entry
  synthesis point (research Pattern 2); limits guard (≤512 files, ≤16 MiB)
  and name-identity validation live at the same choke point.
</specifics>

<deferred>
## Deferred Ideas

- **Stdio transport reach for skills/list + skills/get** (D-01) — owner:
  next skills phase (v2.7 milestone); the seam's rustdoc calls widening a
  non-semver-breaking follow-on.
- **`resources/directory/read`** (spike gap #6) — legal to defer;
  declaration already means `directoryRead: false`.
- **Client wrappers** `list_skills()` / `get_skill()` / `read_skill_uri()`
  (spike gap #7) — additive public API on `Client` (wasm32-compiling);
  defer to a later v2.7 phase.
- **Strict frontmatter mode** (`try_`/strict variant erroring at build) —
  after canonical surfaces are cleaned up (D-03).
- **`resources/read` `-32601` divergence fix** (D-06 observation).
</deferred>

### Research Findings

# Phase 125: SEP-2640 Conformance — skills/list + skills/get — Research

**Researched:** 2026-09-01
**Domain:** MCP protocol extension conformance (Rust SDK) — internal method routing, resource/skill registries, content digests, YAML frontmatter
**Confidence:** HIGH (every in-repo claim read from source this session; SEP draft re-fetched from the PR head branch this session)

> **No CONTEXT.md exists for this phase** (`.planning/phases/125-sep-2640-conformance-skills-list-skills-get/` is empty). There are therefore no locked user decisions. Everything in `## Open Questions` needs `/gsd-discuss-phase 125` before planning locks it.

---

## Summary

The shipped `skills` module auto-declares `io.modelcontextprotocol/skills` and implements neither of the two methods the current SEP-2640 draft makes mandatory. Spike 008 measured that; this research charts the wiring the fix has to touch. Three findings dominate the plan.

**First — the prescribed `InternalClientRequest` route reaches exactly one transport.** The seam's own rustdoc says so, and I confirmed it empirically: `StdioTransport::parse_message` on `{"method":"skills/list"}` returns `Err`, and the server actor's receive arm **breaks the loop** on a receive `Err`. So a `skills/list` over stdio today does not merely answer `-32601` — it tears down the connection. The `server/discover` precedent is safe with HTTP-only reach because `server/discover` is a v2-only method with an era gate; `skills/list` has **no** version gate in the draft (it rides the base Resources primitive, and only the `ttlMs`/`cacheScope` attributes are 2026-07-28-conditional). The phase must therefore decide, explicitly, whether it widens `IngressRequest::Internal` to the generic transport path or ships HTTP-only reach and says so.

**Second — the entry-manifest work is API surface, exactly as the spike said, but with one live tension the spike did not surface.** `Skill` already holds every input (`name`, `body`, `path`, `description`, `references`) and `sha2 = "0.11"` is already a non-optional `pmcp` dependency, so digests and sizes cost nothing. The tension is *verbatim frontmatter*: nearly every existing test, doctest and proptest constructs skills with **no frontmatter at all** (`Skill::new("x", "body")`), while the draft requires `frontmatter.name` and `frontmatter.description` to always be present and to be byte-identical to the SKILL.md a host fetches. A synthesized `{name, description}` for a frontmatter-less skill is a *guaranteed* host-side verification failure, not a graceful default.

**Third — the local quality gate does not see this module.** `skills` is in neither `default` nor `full`; `make lint`, `make test-unit`, `make test-integration` and `make doc-check` all pin `--features full` or an explicit list that omits `skills`. Only `make build` (`--all-features`) and `make test-examples` (`-p pmcp --all-features --examples`) compile it, and neither runs a test. CI's `cargo test --all-features` and `cargo clippy --all-targets --all-features` do cover it. A plan that verifies with `make quality-gate` alone will ship a green gate over untested code.

**Primary recommendation:** Land the two methods on the `InternalClientRequest` route with an explicit, tested decision about stdio reach; compute entries at `into_handler()` from a new crate-private `SkillEntry` carried on the built server (not by downcasting the `ResourceHandler`); take `serde_yaml 0.9` as an **optional** `pmcp` dep gated on `skills` (it is already in `Cargo.lock` and is already a production dep of four workspace crates — zero new packages, and `cargo audit` is clean on it today); and verify with `cargo test --all-features`, never with `make quality-gate` alone.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Method-string classification (`skills/list`, `skills/get`) | Protocol types (`src/types/protocol/mod.rs`) | — | `classify_internal_method` is the single interception point; `src/shared/protocol_helpers.rs` consumes it. Putting the spelling anywhere else creates two places that can disagree (the `SERVER_DISCOVER_METHOD` single-sourcing rustdoc argues exactly this). |
| Ingress routing to a handler | Transport (`src/server/streamable_http_server.rs`) + shared seam (`src/shared/protocol_helpers.rs`) | Generic transport actor (`src/server/mod.rs`) | Today `HttpIngress` is the only production consumer of `IngressRequest::Internal`. Widening reach is a change in the generic actor, not in the skills module. |
| Entry synthesis (frontmatter JSON, digest, size) | Skills module (`src/server/skills.rs`) | — | Every input already lives in `Skill`; `resolved_path()` is `pub(crate)`, so the computation cannot live outside the crate anyway. |
| Answering the two methods | Server (`src/server/mod.rs` `Server`, `src/server/core.rs` `ServerCore`) | — | Mirrors `Server::handle_discover` — a thin delegate over one shared projection fn. Both build paths (`ServerBuilder` and `ServerCoreBuilder`) must carry the entries. |
| Build-time validation (name identity, limits) | Skills module (`Skills::into_handler`) | Builder (`finalize_skills_resources`) | The registry is the only place that sees all skills together; `into_handler()` is already the duplicate-URI gate. |
| Capability declaration (`directoryRead`) | Skills module (`set_skills_capabilities`) | — | One function, four call sites, already single-sourced. |
| wasm32 | *(excluded)* | — | `pub mod skills` is `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]`; `src/server/wasm_core.rs` contains zero occurrences of `skill` or `extensions`. No wasm work. |

---

## Project Constraints (from CLAUDE.md)

These are directives, not suggestions. The planner must verify each plan against them.

| Directive | Source | Consequence for this phase |
|-----------|--------|----------------------------|
| ZERO tolerance for defects; `make quality-gate` before any commit/push | CLAUDE.md "Toyota Way" + trailing bullet | Every plan's verify block runs it — **but see the gate blind-spot finding below; it does not reach this module.** Pair it with `cargo test --all-features`. |
| Cognitive complexity ≤ 25 per function (CI-enforced by PMAT, PR-blocking) | CLAUDE.md "CI Quality Gates" | The entry-synthesis fn and the classifier arms must stay small. PMAT runs only in CI (D-07), so a local pass proves nothing here. |
| Zero SATD comments | CLAUDE.md | No `TODO`/`FIXME` in the deferral of gaps #6/#7 — record deferrals in the phase docs, not in code comments. `make check-todos` is in the gate. |
| ALWAYS requirements for every new feature: **fuzz, property, unit, `cargo run --example`** | CLAUDE.md "ALWAYS Requirements" | `skills/list`/`skills/get` are a new feature. Plan needs: a proptest over entry synthesis, unit tests, and an example update (s44/c10 already exist — extend them rather than adding a third). |
| Doctests must pass; comprehensive rustdoc with examples | CLAUDE.md | New public API (`Skills::entries()` or equivalent) needs rustdoc + a doctest. Note `make doc-check` omits `skills` from its feature list — the doctest is only exercised by `cargo test --doc --all-features`. |
| Contract-first: update contract YAML in `../provable-contracts/contracts/<crate>/`, run `pmat comply check` | CLAUDE.md "Contract-First Development" | `make comply` is in `quality-gate`. Check whether a `pmcp` skills contract exists before editing. |
| Semver: the 2.x promise — no new variants on public exhaustive enums | CLAUDE.md release section + `src/types/protocol/mod.rs:775-790` | Hard constraint, and success criterion #1. See "Don't Hand-Roll". |

---

## Phase Requirements

No formal `REQ-` IDs exist for this phase (ROADMAP says `**Requirements**: TBD`). The working requirement set is the spike-derived gap table. Mapped here so the planner can trace coverage:

| Gap | Severity | Description | Research support |
|-----|----------|-------------|------------------|
| #1 | CRITICAL | `skills/list` + `skills/get` unrouteable while capability auto-declared | `## Architecture Patterns` Pattern 1 (the classifier route) + `## Common Pitfalls` Pitfall 1 (transport reach) |
| #2 | MAJOR | No entry-manifest API | Pattern 2 (entry synthesis) + `## Standard Stack` (sha2 already present; YAML decision) |
| #3 | MAJOR | `skill://index.json` nonstandard | Pattern 3 (retirement blast radius — 14 assertion sites enumerated) |
| #4 | MINOR | No name-identity validation | Pitfall 3 (30+ frontmatter-less call sites would break under a strict rule) |
| #5 | MINOR | No 512-file / 16 MiB limit guard | Pattern 2 (limits are computable from the entry alone) |
| #6 | INFO | `resources/directory/read` unimplemented | Open Question 4 (capability shape; `directoryRead: false` is the honest declaration) |
| #7 | INFO | No client wrappers | Open Question 5 |

---

## Standard Stack

### Core — already in the tree, no new dependency needed

| Library | Version | Purpose | Why standard |
|---------|---------|---------|--------------|
| `sha2` | `0.11` | `sha256:{64 lowercase hex}` digests over each file's raw bytes | **Already a non-optional `pmcp` dependency** — `Cargo.toml:149` reads `sha2 = "0.11"` in the `# OAuth dependencies` block. Five in-tree `use sha2::{Digest, Sha256};` sites already exist (`src/types/mrtr.rs:69`, `src/server/request_state.rs:92`, `src/server/auth/oauth2.rs:437`, `src/shared/pkce.rs:52`, `src/client/oauth.rs:20`). [VERIFIED: Cargo.toml:149] |
| `indexmap` | in tree | Deterministic entry ordering | `Skills::into_handler` already builds `IndexMap<String, Skill>` for exactly this reason (`src/server/skills.rs:439-440`). [VERIFIED: src/server/skills.rs:438-440] |
| `serde_json` | in tree | Rendering the frontmatter object and the entry | Already the module's serialization path. |

### Supporting — the one real decision

| Library | Version | Purpose | When to use |
|---------|---------|---------|-------------|
| `serde_yaml` | `0.9.34+deprecated` | Parse the SKILL.md frontmatter block into a `serde_json::Value` so `frontmatter` is verbatim (nested maps, lists, `metadata` objects) | **Recommended.** Already resolved in `Cargo.lock` at `0.9.34+deprecated` (one entry) and already a production dep of `crates/mcp-tester`, `crates/pmcp-server-toolkit` (optional), `crates/pmcp-team-servers` (optional) and `crates/pmcp-sql-server`. Adding it as an **optional** `pmcp` dep under the `skills` feature adds **zero** new packages to the graph. [VERIFIED: Cargo.lock:6654-6664 — `name = "serde_yaml"` / `version = "0.9.34+deprecated"` / deps `indexmap 2.14.0`, `itoa`, `ryu`, `serde`, `unsafe-libyaml`] |

### Alternatives Considered

| Instead of | Could use | Tradeoff |
|------------|-----------|----------|
| `serde_yaml` 0.9 | `serde_yaml_ng` 0.10.0 | Maintained *fork* of serde_yaml, same API. **But**: last published `0.10.0` on 2024-05-26 — no release in ~2 years [VERIFIED: crates.io API, `/api/v1/crates/serde_yaml_ng/versions` returns exactly `0.10.0` (2024-05-26), `0.9.36`, `0.9.35`]. Adds a NEW package to the lock. Legitimacy check: `OK`, repo `github.com/acatton/serde-yaml-ng`. |
| `serde_yaml` 0.9 | ~~`serde-yml`~~ / `serde_yml` | **REJECTED — refuted this session.** The crate is `serde_yml` (underscore), and its own crates.io description now reads: `DEPRECATED — 'serde_yml' is unmaintained. This release is a thin compatibility shim that forwards every call to 'noyalib'` [VERIFIED: crates.io API `/api/v1/crates/serde_yml`, `max_stable_version` 0.0.13, updated 2026-05-27]. Spike 008 listed it as a candidate; that recommendation is stale. |
| `serde_yaml` 0.9 | `saphyr` 0.0.12 | A genuine, actively-maintained YAML 1.2 parser (updated 2026-08-18, 744k recent downloads) — but it is a **parser, not a serde data format**. There is no usable serde bridge: `saphyr-serde` on crates.io is `0.0.0`, description `"tmp"`, no repository, 38 recent downloads [VERIFIED: crates.io API `/api/v1/crates/saphyr-serde`]. Choosing `saphyr` means hand-writing the YAML→JSON conversion. Use only if a maintained-dependency policy forbids `serde_yaml`. |
| A YAML dep at all | Keep the shipped line-scanner and document a **flat-frontmatter limit** | Zero dependency cost, but the draft is explicit that `metadata` is an *object* and that "everything else … passes through unchanged" (SEP §Frontmatter). A flat-only parser silently drops nested fields, and the draft requires hosts to compare field-by-field and **refuse the skill** on any discrepancy. This path ships a known host-side rejection for any skill with a `metadata:` block. Viable only if the phase also *rejects* non-flat frontmatter at build time rather than silently flattening it. |

**Installation** (if the recommendation is taken):

```toml
# Cargo.toml [dependencies]
serde_yaml = { version = "0.9", optional = true }

# Cargo.toml [features]
skills = ["dep:serde_yaml"]
```

**Version verification performed this session:**
```bash
curl -s -H 'User-Agent: ...' https://crates.io/api/v1/crates/<name>     # registry metadata
node ~/.claude/gsd-core/bin/gsd-tools.cjs query package-legitimacy check --ecosystem crates ...
cargo audit                                                              # exit 0, 7 allowed warnings
```

---

## Package Legitimacy Audit

| Package | Registry | Age | Downloads (recent 90d) | Source Repo | Verdict | Disposition |
|---------|----------|-----|------------------------|-------------|---------|-------------|
| `sha2` | crates.io | since 2016-05-06 | 18,193,153 wk | github.com/RustCrypto/hashes | OK | Approved — **already a dependency**, no install needed |
| `serde_yaml` | crates.io | since 2016-02-27 | 6,833,229 wk | github.com/dtolnay/serde-yaml | OK | Approved (recommended) — already in `Cargo.lock` |
| `serde_yaml_ng` | crates.io | since 2024-05-03 | 447,522 wk | github.com/acatton/serde-yaml-ng | OK | Alternative — approved but adds a package |
| `saphyr` | crates.io | since 2024-04-02 | 57,932 wk | github.com/saphyr-rs/saphyr | OK | Alternative — no serde bridge |
| `serde_yml` | crates.io | — | — | github.com/sebastienrousseau/serde_yml | — | **REMOVED** — self-declared deprecated/unmaintained shim |
| `saphyr-serde` | crates.io | 2024-04-02, still `0.0.0` | 38 (90d) | none | SUS | **REMOVED** — placeholder crate, description `"tmp"`, no repo |

**Packages removed:** `serde_yml`, `saphyr-serde`.
**Packages flagged suspicious:** `saphyr-serde` (removed rather than gated).
**Postinstall check:** N/A for the crates ecosystem — but `serde_yaml` pulls `unsafe-libyaml` (a C-to-Rust transpile of libyaml). It is already in the graph today, so this is not a new exposure.

**`cargo audit` measurement (run this session, exit 0):** `warning: 7 allowed warnings found`. The flagged crates are `paste`, `smartstring`, `anyhow`, `event-listener`, `lru`, `rand`, `chacha20`. **Neither `serde_yaml` nor `unsafe-libyaml` appears** — there is no RUSTSEC advisory against them in the current advisory DB, notwithstanding the `+deprecated` version suffix. `deny.toml:11-19` carries six `ignore` entries; none relates to YAML.

---

## Architecture Patterns

### System Architecture Diagram

```
                          ┌──────────────────────────────────────┐
  JSON-RPC frame          │  shared/protocol_helpers.rs          │
  {"method":"skills/list"}│  parse_request_or_internal()         │
        │                 │    ├─ classify_internal_method(m,p)  │  ← ONE interception point
        │                 │    │    (types/protocol/mod.rs:873)  │
        ▼                 │    │                                  │
  ┌───────────┐           │    ├─ Some(_) → IngressRequest::Internal
  │ Transport │──────────▶│    └─ None   → IngressRequest::Public(Request)
  └───────────┘           └──────────────────┬───────────────────┘
        │                                     │
        │            ┌────────────────────────┴─────────────────────────┐
        │            ▼                                                   ▼
        │   ┌──────────────────────────┐                    ┌────────────────────────┐
        │   │ streamable_http_server   │                    │ PUBLIC parse_request() │
        │   │ classify_http_ingress()  │                    │  (helpers.rs:110-116)  │
        │   │  → HttpIngress::Discover │                    │  Internal(_) =>        │
        │   │  → HttpIngress::TasksUpd │                    │    method_not_found    │
        │   │  ★ → HttpIngress::Skills*│                    └───────────┬────────────┘
        │   └────────────┬─────────────┘                                │
        │                │                                              ▼
        │                ▼                              ┌──────────────────────────────┐
        │   ┌──────────────────────────┐                │ shared/transport.rs:138      │
        │   │ Server::handle_discover  │                │ parse_method_message wraps   │
        │   │ ★ Server::handle_skills_*│                │ the Err as InvalidMessage    │
        │   └────────────┬─────────────┘                └───────────┬──────────────────┘
        │                │                                          ▼
        │                ▼                              ┌──────────────────────────────┐
        │   ┌──────────────────────────┐                │ run_transport_actor          │
        │   │ ★ skills_list_response() │                │ (server/mod.rs:1451-1470)    │
        │   │   ★ skills_get_response()│                │ Err(e) => log_error; BREAK   │
        │   │   ONE shared projection  │                │  ▲ THE STDIO CLIFF           │
        │   └────────────┬─────────────┘                └──────────────────────────────┘
        │                │
        │                ▼
        │   ┌───────────────────────────────────────────────────┐
        │   │ ★ SkillEntry[] — computed ONCE at into_handler()  │
        │   │   {uri, frontmatter: Value, resources: [{uri,     │
        │   │    digest:"sha256:<64hex>", size}]}               │
        │   │   carried on Server / ServerCore beside `resources`│
        │   └───────────────────────────────────────────────────┘
                              ▲
                              │  built from
        ┌─────────────────────┴──────────────────────┐
        │  Skills registry (server/skills.rs)        │
        │  Skill{name, body, path, description,      │
        │        references: Vec<SkillReference>}    │
        └────────────────────────────────────────────┘

  ★ = new in this phase
```

### Recommended change surface

```
src/types/protocol/mod.rs      # + SKILLS_LIST_METHOD / SKILLS_GET_METHOD consts
                               # + InternalClientRequest::{SkillsList, SkillsGet} (pub(crate))
                               # + classify_internal_method arms
src/shared/protocol_helpers.rs # (no change if HTTP-only; widen if stdio reach is chosen)
src/server/streamable_http_server.rs
                               # + HttpIngress::{SkillsList, SkillsGet}
                               # + classify_http_ingress fast-reject spellings
                               # + the two per-path response-assembly arms (5 sites, see below)
src/server/skills.rs           # + SkillEntry / frontmatter extraction / digest+size
                               # + Skills::entries() (or entries computed in into_handler)
                               # + name-identity + limits validation
                               # - SKILL_INDEX_URI + build_discovery_index_json (retire/gate)
src/server/mod.rs              # + entries field on Server; + handle_skills_list/get delegates
src/server/core.rs             # + entries field on ServerCore; + the shared projection fn
src/server/builder.rs          # + entries threaded through finalize_skills_resources
Cargo.toml                     # skills = ["dep:serde_yaml"]
examples/s44_server_skills.rs  # index.json lines; add skills/list demo
examples/c10_client_skills.rs  # index.json read → skills/get
tests/skills_integration.rs    # index assertions; + entry-shape assertions
tests/<new>_skills_routing.rs  # the semver tripwire + wire proofs
```

### Pattern 1: The `InternalClientRequest` classifier route (the prescribed path)

**What:** Add the two method spellings to a crate-private enum and a `match`, never to the public `ClientRequest`.
**When to use:** Any new wire method during the 2.x window.
**Anatomy, as it stands today (all line-cited, read this session):**

`src/types/protocol/mod.rs:768-771` — the enum is `pub(crate)`, so it is invisible to `cargo-semver-checks` / `cargo-public-api`:
```rust
#[derive(Debug, Clone)]
pub(crate) enum InternalClientRequest {
    /// The v2 `server/discover` request (VERS-04).
    ServerDiscover(ServerDiscoverRequest),
```

`src/types/protocol/mod.rs:873-886` — the classifier is a bare method-string `match` that **never deserializes `params`**:
```rust
pub(crate) fn classify_internal_method(
    method: &str,
    params: &serde_json::Value,
) -> Option<InternalClientRequest> {
    match method {
        SERVER_DISCOVER_METHOD => Some(InternalClientRequest::ServerDiscover(
            ServerDiscoverRequest::new(),
        )),
        TASKS_UPDATE_METHOD => Some(InternalClientRequest::TasksUpdate {
            params: params.clone(),
        }),
        _ => None,
    }
}
```

`src/shared/protocol_helpers.rs:42-55` — the ingress enum, and note the `cfg_attr`, which tells you where the only reader lives:
```rust
pub(crate) enum IngressRequest {
    /// A public typed request (the existing exhaustive-enum dispatch path).
    Public(Request),
    /// An internally-routed method with no public enum variant (v2-only).
    #[cfg_attr(
        any(target_arch = "wasm32", not(feature = "streamable-http")),
        allow(dead_code)
    )]
    Internal(crate::types::protocol::InternalClientRequest),
}
```

`src/shared/protocol_helpers.rs:110-116` — the PUBLIC entrypoint maps `Internal` back to `-32601`:
```rust
pub fn parse_request(request: JSONRPCRequest<Value>) -> Result<(RequestId, Request)> {
    let method = request.method.clone();
    match parse_request_or_internal(request)? {
        (id, IngressRequest::Public(req)) => Ok((id, req)),
        (_, IngressRequest::Internal(_)) => Err(Error::method_not_found(&method)),
    }
}
```

**Where classification happens on the wire (HTTP):** `classify_http_ingress` at `src/server/streamable_http_server.rs:2260-2318`, with a fast-reject that pins the method spellings *before* calling the shared seam. Its inner `match` over `InternalClientRequest` is **deliberately exhaustive** — its comment says "adding a future internally-routed method is a compile-time tripwire here." That is a feature: adding the two variants will break this build until the arms are written.

**Where the response is assembled (HTTP):** five sites, all in `streamable_http_server.rs` — `3243-3247`, `3785`, `4940`, `5022`, `5135` — plus `HttpIngress::is_initialize` at `~2242-2255`, which must return `false` for the new variants. Each new `HttpIngress` variant must be handled at every one of them.

**Where `-32601` is produced for the era-gated case:** `build_discover_response` (`src/server/core.rs:2380`) called through the thin `Server::handle_discover` delegate (`src/server/mod.rs:1657-1675`). **This is the shape to copy, but not the gate:** `server/discover` is v2-only and answers `-32601` on v1; `skills/list` has no such gate.

### Pattern 2: Entry synthesis at `into_handler()`

**What:** Compute the complete `SkillEntry` set once, at build time, from the registry.
**Why here:** `Skills::into_handler` (`src/server/skills.rs:437-471`) is already the single place that sees every skill together, already errors on duplicate URIs, and already builds the deterministic `IndexMap`. `Skill::resolved_path()` is `pub(crate)` (`src/server/skills.rs:280-282`), so entry synthesis *cannot* live outside the crate.

Verbatim from `src/server/skills.rs:280-286`:
```rust
    pub(crate) fn resolved_path(&self) -> &str {
        self.path.as_deref().unwrap_or(&self.name)
    }

    pub(crate) fn skill_md_uri(&self) -> String {
        format!("skill://{}/SKILL.md", self.resolved_path())
    }
```

**The data model is sufficient** — `src/server/skills.rs:155-162`:
```rust
#[derive(Clone, Debug)]
pub struct Skill {
    name: String,
    body: String,
    path: Option<String>,
    description: String,
    references: Vec<SkillReference>,
}
```

**Digest and size:** `digest = "sha256:" + hex(Sha256(bytes))`, `size = bytes.len()`. The bytes are exactly what `resources/read` returns — `skill.body()` for SKILL.md, `reference.body()` for each supporting file — so the manifest and the served content cannot disagree by construction. Existing sha2 idiom in-repo (`src/server/request_state.rs:223-226`):
```rust
    let mut hasher = Sha256::new();
    hasher.update(key);
    let digest = hasher.finalize();
```
**No `hex` crate is in `pmcp`'s `[dependencies]`** (grep: zero `^hex =` matches in `Cargo.toml`), and no in-repo `pmcp` site formats a digest as hex today. `sha2` is at **0.11** (`digest` 0.11) in this workspace, *not* the 0.10 the spike used, so the spike's `format!("sha256:{:x}", h.finalize())` is **not** a safe copy-paste — verify the `LowerHex` impl exists on `digest` 0.11's output type, or write `{:02x}` over the byte slice. **This is a real, cheap trap: confirm it in Wave 0.**

**Limits are checkable from the entry alone** (SEP §Limits): count `resources` entries against 512, sum `size` against 16,777,216. Guard at `into_handler()`, per gap #5.

### Pattern 3: Retiring `skill://index.json` — the blast radius

The index is defined at `src/server/skills.rs:56-60`:
```rust
/// Synthesized discovery-index URI; emitted in `resources/list` and
/// served from `resources/read`.
const SKILL_INDEX_URI: &str = "skill://index.json";
const SKILL_MD_MIME: &str = "text/markdown";
const INDEX_JSON_MIME: &str = "application/json";
```
…synthesized by `build_discovery_index_json` (`src/server/skills.rs:514-530`) against `"$schema": "https://schemas.agentskills.io/discovery/0.2.0/schema.json"`, pushed into `list_resources` in `SkillsHandler::new` (`:499-503`), and short-circuited in `read` (`:544-550`).

**Every assertion site that changes when it retires** (measured by grep this session):

| File | Lines | What breaks |
|------|-------|-------------|
| `src/server/skills.rs` | 804, 826, 910-915, 972-995, 1108-1111, 1240, 1392 | 7 unit-test assertions on list length / index position / index read |
| `tests/skills_integration.rs` | 168-188 (`resources_list_returns_skill_md_and_index_only`, incl. `assert_eq!(result.resources.len(), 3, "2 SKILL.md + 1 index = 3")`), 225 (`resources_read_index_returns_resource_with_text_application_json`), 351-380 (proptest reads `"skill://index.json"` in its URI loop) | 3 sites, one of them a proptest |
| `examples/s44_server_skills.rs` | doc header line ~19 + `println!("Also auto-synthesized: skill://index.json");` | example output text |
| `examples/c10_client_skills.rs` | 107-114 (`.read("skill://index.json", …)` + two `assert_eq!`) | example **asserts** on it — it will panic, not just print wrong |
| `pmcp-book/src/ch12-8-skills.md` | 4 occurrences | doc drift; `make book-test` runs `mdbook test` |
| `pmcp-course/src/part8-advanced/ch23-skills.md` | 2 occurrences | doc drift |
| `pmcp-course/src/part8-advanced/ch23-exercises.md` | 3 occurrences | doc drift |

`src/server/skills.rs:19` states the module doctest is a **"Byte-equal mirror of the doctest at the end of `pmcp-book/src/ch12-8-skills.md`"** — so the book chapter and the module doc move together by rule.

### Anti-Patterns to Avoid

- **Adding `SkillsList` / `SkillsGet` variants to `ClientRequest`.** `src/types/protocol/mod.rs:777-780` records the measurement: the enum carries `#[serde(tag = "method", content = "params", rename_all = "camelCase")]` "with **no `#[non_exhaustive]`**", so `enum_variant_added` is a semver-MAJOR break. Success criterion #1 is exactly this.
- **Adding `#[non_exhaustive]` to `ClientRequest` as an escape hatch.** Same rustdoc rejects it: "that is itself a source break for every downstream exhaustive `match`."
- **Downcasting the `ResourceHandler` to reach the entries.** `finalize_skills_resources` (`src/server/builder.rs:1434-1452`) may wrap the skills handler in `ComposedResources` when the author also called `.resources(...)`. Any downcast has to know about that wrapper, and will silently return "no skills" the day a third composition layer appears. Carry the entries as their own field.
- **Reconstructing frontmatter from `Skill::resolved_description()`.** `with_description` is an explicit *override* (`src/server/skills.rs:190-195`), so `resolved_description()` can legitimately differ from the SKILL.md's `description:` line. The draft requires the emitted `frontmatter` to be identical to the file's. Emit from the parsed frontmatter block, never from the resolved field.
- **Rebuilding the discovery index in a new shape.** SEP §Discovery: the WG chose a method. `skill://index.json` also violates the URI rule (`index.json` is not a skill name and `skill://index.json/SKILL.md` does not exist).

---

## Don't Hand-Roll

| Problem | Don't build | Use instead | Why |
|---------|-------------|-------------|-----|
| Routing a new wire method without a public enum variant | A parallel dispatch table, or a `_ =>` fallthrough in `parse_request` | `classify_internal_method` + `InternalClientRequest` | Two spellings that can disagree is precisely what the `SERVER_DISCOVER_METHOD` single-sourcing rustdoc exists to prevent, and the `classify_http_ingress` inner `match` is already a compile-time tripwire for new variants. |
| YAML → JSON for verbatim frontmatter | A hand-rolled `k: v` line splitter | `serde_yaml` (or an accepted alternative) | The shipped scanner (`parse_frontmatter_description`, `src/server/skills.rs:644-664`) reads only `description: ` and only in the first 40 lines. Nested maps, block scalars, lists, quoting and anchors all exist in real SKILL.md frontmatter, and the draft makes a host-side field-by-field mismatch a hard load failure. |
| SHA-256 | Anything | `sha2` (already a dependency) | Five existing in-repo call sites. |
| Deterministic entry ordering | Sorting at response time | `IndexMap`, as `into_handler` already does | Insertion order is already the module's documented contract (`src/server/skills.rs:8-10`). |
| A semver regression check | Reading the diff carefully | The in-repo source-scanning tripwire idiom | `tests/v2_tasks_update_routing.rs:1196-1208` (`client_request_has_no_tasks_update_variant`) reads `src/types/protocol/mod.rs`, locates `\npub enum ClientRequest {`, and scans the block. **There is no `cargo semver-checks` in `Makefile` or `.github/workflows/` — grep returned zero hits.** The tripwire test *is* the enforcement. |

**Key insight:** every "hard" part of this phase already has an in-repo precedent that was argued out at length in rustdoc. The work is following four established patterns, not inventing one.

---

## Common Pitfalls

### Pitfall 1: The `InternalClientRequest` route reaches only streamable HTTP — and on stdio it kills the connection

**What goes wrong:** `skills/list` is implemented, HTTP tests pass, and a stdio host's first `skills/list` terminates the server.

**Why it happens:** The seam's own rustdoc says it (`src/shared/protocol_helpers.rs:32-42`, verbatim):

> "The ONLY production consumer of [`IngressRequest::Internal`] is `classify_http_ingress` in `src/server/streamable_http_server.rs`. Every other transport reaches requests through the PUBLIC [`parse_request`], which maps `Internal` to [`Error::method_not_found`] — so an internally-routed method is served over streamable HTTP and answers `-32601` everywhere else, including stdio."

And it is worse than `-32601` in practice, because the generic transport never gets a chance to answer. `src/shared/transport.rs:138-139` turns the parse failure into a transport error:
```rust
        let parsed_request = crate::shared::parse_request(request)
            .map_err(|e| TransportError::InvalidMessage(format!("Invalid request: {}", e)))?;
```
and `src/server/mod.rs:1463-1466` breaks the actor loop on any receive error:
```rust
                        Err(e) => {
                            Self::log_error(&format!("Transport receive error: {}", e)).await;
                            break;
                        },
```

**MEASURED this session** (scratch binary, `pmcp` path dep, `default-features = false, features = ["skills"]`, calling the public `pmcp::shared::StdioTransport::parse_message`):
```
skills/list                      => Err(Transport error: Invalid message format: Invalid request: Protocol error: -32601 - Method not found: skills/list)
skills/get                       => Err(Transport error: Invalid message format: Invalid request: Protocol error: -32601 - Method not found: skills/get)
resources/directory/read         => Err(Transport error: Invalid message format: Invalid request: Protocol error: -32601 - Method not found: resources/directory/read)
totally/unknown                  => Err(Transport error: Invalid message format: Invalid request: Protocol error: -32601 - Method not found: totally/unknown)
resources/list                   => Ok(TransportMessage)
```

The `totally/unknown` control shows this is **pre-existing behaviour for every unroutable method**, not something this phase introduces. But it means routing `skills/list` internally buys **zero** stdio reach, and stdio is the transport `examples/s44`/`c10` and most local hosts use.

**How to avoid:** Make it an explicit, planned decision (see Open Question 1), and put a test on whichever answer is chosen. The rustdoc already blesses widening: "The seam is transport-AGNOSTIC (it lives in `shared/`), so a later plan can widen the reach without a semver break."

**Warning signs:** an HTTP-only integration test suite; a plan whose only wire proof is `classify_internal_method` returning `Some`.

### Pitfall 2: `make quality-gate` never compiles or tests the skills module

**What goes wrong:** The phase lands, `make quality-gate` is green, CI fails — or worse, CI is green on clippy/test but nothing local ever exercised the code the phase wrote.

**Why it happens:** `Cargo.toml:306` reads `skills = []` and the feature appears in **neither** `default` (`["logging", "v1-compat"]`) **nor** `full`. Measured coverage of each gate leg:

| Gate leg | Command | Reaches `skills`? |
|----------|---------|-------------------|
| `make lint` | `cargo clippy --features "full" --lib --tests …` then `cargo check --features "full" --examples` | **NO** |
| `make test-unit` | `cargo test --lib --features "full"` | **NO** — `src/server/skills.rs` unit tests never run |
| `make test-integration` | `cargo test --test '*' --features "full"` | **NO** — `tests/skills_integration.rs` opens with `#![cfg(all(feature = "skills", not(target_arch = "wasm32")))]` (line 27), so it compiles to zero tests |
| `make test-doc` | `cargo test --doc --features "full"` | **NO** — the `skills.rs` doctests never run |
| `make doc-check` | `cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket,v1-compat` | **NO** — `skills` is absent from that explicit list, so new rustdoc on the module is never warning-checked locally |
| `make build` | `cargo build --all-features` | YES (compiles only) |
| `make test-examples` | `scripts/run-example-builds.sh` → `cargo build -p pmcp --all-features --examples` | YES (builds s44/c10; does not run them) |
| CI `.github/workflows/ci.yml:63` | `cargo clippy --all-targets --all-features` | YES |
| CI `.github/workflows/ci.yml:104` | `cargo test --all-features --verbose -- --test-threads=1` | YES |
| CI `.github/workflows/ci.yml:113` | `cargo test --doc --all-features` | YES |

`grep -rn "skills" .github/workflows/*.yml` returns **zero** matches — CI covers this module only incidentally, via `--all-features`.

**How to avoid:** Every plan's verify block runs `cargo test --all-features -- --test-threads=1` (matching CI) **in addition to** `make quality-gate`. Consider a plan task that adds `skills` to the `full` feature list or adds a dedicated Makefile leg — but note `full` and `full-v2` are two enumerated lists whose drift is itself a test failure (`tests/v1_severability_tripwire.rs` derives both from `Cargo.toml`), so touching `full` is not free.

**Warning signs:** a verify block whose only command is `make quality-gate`; `0 tests` in a run's output.

### Pitfall 3: Strict name-identity validation breaks ~30 existing call sites

**What goes wrong:** Gap #4 is implemented as "frontmatter `name` must equal the final URI segment", and the workspace stops compiling/passing.

**Why it happens:** the overwhelming majority of in-repo skills have **no frontmatter at all**. Measured (`grep -rn 'Skill::new(' src/ tests/ examples/ crates/`): 40+ call sites, of which the module's own doctests use `Skill::new("x", "body")` (`src/server/skills.rs:216, 244, 248, 306`), `Skill::new("a", "body-a")` (`:381-382`), and the unit tests use `Skill::new("a", "")`, `Skill::new("foo", "body")`, `Skill::new("zeta", "")` … The proptest strategy at `src/server/skills.rs:1116-1140` generates `name` from `"[a-z]{1,8}"` and `body` from `"[a-zA-Z]{0,20}"` — arbitrary bodies that will essentially never contain valid frontmatter. `tests/skills_integration.rs:319-350` does the same with `Skill::new("propskill", body)`.

Note the two distinct sub-gaps the spike separated:
- **4a** — `with_path("acme/billing")` on a skill named `refunds` yields `skill://acme/billing/SKILL.md`; final segment `billing` ≠ `refunds`. Checkable against `Skill::name()` alone, and **breaks nothing existing** (`examples/s44` uses `.with_path("acme/billing/refunds")`, whose final segment is correct).
- **4c** — `Skill::new("something-else", body-whose-frontmatter-says-refunds)`. Checkable only when frontmatter exists.

**How to avoid:** implement 4a unconditionally against `Skill::name()` (cheap, zero blast radius) and implement 4c **conditionally** — only when the body actually carries a frontmatter block with a `name` key. Do not require frontmatter to exist at construction.

**Warning signs:** a plan task worded "validate frontmatter name" without a "when frontmatter is present" clause; proptest failures with shrunk inputs like `name = "a", body = ""`.

### Pitfall 4: A frontmatter-less skill cannot produce a conforming `skills/list` entry

**What goes wrong:** `Skills::entries()` synthesizes `{"name": skill.name(), "description": skill.resolved_description()}` for a skill with no frontmatter. A conforming host fetches the SKILL.md, parses zero frontmatter, compares field-by-field against the entry, finds a discrepancy, and — per SEP §Integrity and verification — **MUST NOT load the skill**. The server looks conformant and is unusable.

**Why it happens:** the draft is unambiguous (SEP §Frontmatter, line 239): "`frontmatter` is the skill's `SKILL.md` YAML frontmatter rendered verbatim as a JSON object — every field the author wrote, not a curated subset," and (line 241) "The `frontmatter` object MUST be identical in content to the frontmatter of the `SKILL.md` it describes." Line 269 makes the host-side check mandatory.

**How to avoid:** decide, and record, what a frontmatter-less skill does — options in Open Question 2. Whatever is chosen, do not silently synthesize.

**Warning signs:** an `entries()` implementation with an `unwrap_or_default()` on the frontmatter parse.

### Pitfall 5: `resultType`, `ttlMs` and `cacheScope` on the results

**What goes wrong:** the wire result omits `resultType` / `ttlMs` / `cacheScope` on a 2026-07-28 connection and fails a conformance check.

**Why it happens:** the draft's examples carry `"resultType": "complete"` on **both** `skills/list` and `skills/get` results (SEP lines 132 and 306), and §Dependencies (line 35) says "In protocol versions 2026-07-28 and later, `skills/list` results additionally carry the base protocol's list-caching attributes ([SEP-2549])". §`skills/list` line 229 names them: `ttlMs` and `cacheScope`. §`skills/get` line 359 explicitly **leaves the `skills/get` case open**: "whether the result should also carry the base protocol's caching attributes … is left open."

In pmcp these are injected by the v2 envelope machinery, and `request_is_cacheable` (`src/server/core.rs:2153-2200`) is keyed on `ClientRequest` variants — which `skills/list` will not have. The rustdoc there tells you exactly what to do:

> "`server/discover` is deliberately absent … it does not ride the `ClientRequest` route at all — `server/discover` is carried by the crate-private `InternalClientRequest` and answered by [`build_discover_response`], which **names `Cacheable::Yes` at its own call site**."

Also note `src/types/mrtr.rs:112` records that "a result with no `resultType` at all is a complete result" — so omission is tolerated, but emitting it matches the draft's examples.

**How to avoid:** name `Cacheable::Yes` at the `skills/list` projection call site, exactly as `build_discover_response` does. Do **not** add a row to `request_is_cacheable` — its rustdoc calls that "a lie about where the claim is made", and its `match` has no wildcard arm, so it will not even compile a bogus row.

### Pitfall 6: Two build paths, two places to thread the entries

**What goes wrong:** `Server::builder()` servers answer `skills/list`; `ServerCoreBuilder` servers return `-32601`, or vice versa.

**Why it happens:** the skills API is wired onto **both** builders, by explicit design decision (80-REVIEWS.md Fix 2, cited in `examples/s44_server_skills.rs:9-13`). `ServerBuilder::skills` lives at `src/server/mod.rs:4520-4528`; `ServerCoreBuilder::skills` at `src/server/builder.rs:479-489`. Both finalize through the shared `finalize_skills_resources` (`src/server/builder.rs:1434`), but each assigns to its own struct's `resources` field (`src/server/mod.rs:5370-5374`, `src/server/builder.rs:1356-1359`). `ServerCore`'s field is `resources: Option<Arc<dyn ResourceHandler>>` at `src/server/core.rs:475-476`.

Note also the cfg asymmetry: `pub mod skills` is `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` (`src/server/mod.rs:194`) while the `ServerBuilder` methods are plain `#[cfg(feature = "skills")]` (`src/server/mod.rs:4501`). Preserve whichever gate each site already uses; do not "harmonize" them in this phase.

**How to avoid:** have `finalize_skills_resources` return the entries alongside the handler, so both call sites get them from one function.

---

## Code Examples

### The current `skills/list` / `skills/get` wire shape (SEP-2640, PR #2640 head `sep/skills-extension`, fetched 2026-09-01)

```json
{
  "jsonrpc": "2.0", "id": 4,
  "result": {
    "resultType": "complete",
    "skills": [
      {
        "uri": "skill://acme/billing/refunds/SKILL.md",
        "frontmatter": {
          "name": "refunds",
          "description": "Process customer refund requests per company policy",
          "license": "Apache-2.0"
        },
        "resources": [
          { "uri": "skill://acme/billing/refunds/SKILL.md",        "digest": "sha256:b2c3d4e5...", "size": 3871 },
          { "uri": "skill://acme/billing/refunds/examples/email.md","digest": "sha256:c3d4e5f6...", "size": 962 }
        ]
      }
    ]
  }
}
```
[CITED: raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/sep/skills-extension/seps/2640-skills-extension.md lines 127-211]

```json
{ "jsonrpc": "2.0", "id": 5, "method": "skills/get",
  "params": { "uri": "skill://pdf-processing/SKILL.md" } }
```
```json
{ "jsonrpc": "2.0", "id": 5,
  "result": { "resultType": "complete", "skill": { "uri": "...", "frontmatter": {...}, "resources": [...] } } }
```
[CITED: same file, lines 290-348]

**Request params (confirmed against the draft, correcting/extending spike 008's capture):**

| Method | Params | Result key | Pagination |
|--------|--------|-----------|------------|
| `skills/list` | optional `cursor` (draft's example shows `"params": {}`) | `skills` (array) | `nextCursor`; "An entry is atomic — a skill's `resources` set is never split across pages" (line 227) |
| `skills/get` | **required** `uri` — "MUST be the URI of a skill's `SKILL.md`" (line 351) | `skill` (single entry, "identical in shape and meaning to an entry of `skills/list`") | none — "The result carries no pagination cursor: a single entry is not a list" (line 359) |

**Error semantics (line 355):** "If the URI does not identify a skill the server serves, the server MUST return error **`-32602`** (Invalid params) — the same code `resources/read` uses for unknown resources." **Note the shipped `SkillsHandler::read` returns `ErrorCode::METHOD_NOT_FOUND` for an unknown URI** (`src/server/skills.rs:556-559`) — that is `-32601`, and it is a *pre-existing* divergence from the draft's stated `resources/read` convention. Do not copy it into `skills/get`.

**Capability declaration (lines 371-389):**
```json
{ "capabilities": { "extensions": { "io.modelcontextprotocol/skills": { "directoryRead": true } } } }
```
| Setting | Type | Default | Meaning |
|---------|------|---------|---------|
| `directoryRead` | boolean | `false` | The server implements `resources/directory/read` |

> "An empty object indicates support for the extension with no optional features. Declaring the extension itself commits the server to `skills/list` and `skills/get`; clients MUST NOT call `resources/directory/read` against a server that has not declared `directoryRead: true`." (line 389)

The shipped declaration is exactly `json!({})` — `src/server/skills.rs:72-75`:
```rust
    caps.extensions
        .get_or_insert_with(HashMap::new)
        .entry(SKILLS_EXTENSION_KEY.to_string())
        .or_insert_with(|| json!({}));
```
That empty object is **already correct** for `directoryRead: false`. The extension key is single-sourced at `src/server/skills.rs:54`:
```rust
pub(crate) const SKILLS_EXTENSION_KEY: &str = "io.modelcontextprotocol/skills";
```
No capability-shape change is required by gap #6.

**Limits (SEP lines 275-278):**

| Limit | Value | Counted over |
|-------|-------|--------------|
| Resources per skill | 512 entries | The entries of the skill's `resources`, `SKILL.md` included |
| Total file size per skill | 16 MiB (16,777,216 bytes) | The sum of `size` over the skill's `resources` |

**Digest format (SEP line 263):** "Digests are SHA-256 hashes of an artifact's raw bytes, formatted as `sha256:{hex}` where `{hex}` is 64 lowercase hexadecimal characters."

**SDK guidance (SEP line 548):** "The SDK handles: reading `SKILL.md` frontmatter to populate resource metadata, serving file content on `resources/read`, and answering `skills/get` — and, **where the server's skill set is bounded**, `skills/list` — computing entry digests and sizes from the registered files, and **warning when a registered skill exceeds the Limits**." pmcp's registry is always bounded (a `Vec<Skill>`), so both methods are in scope.

### The tripwire idiom to copy for success criterion #1

`tests/v2_tasks_update_routing.rs:1196-1208`:
```rust
fn client_request_has_no_tasks_update_variant() {
    let path = repo_root().join("src/types/protocol/mod.rs");
    let source = fs::read_to_string(&path).expect("protocol/mod.rs is readable");

    let start = source
        .find("\npub enum ClientRequest {")
        .expect("the `pub enum ClientRequest` declaration still exists");
    let rest = &source[start + 1..];
    let end = rest
        .find("\n}\n")
        .expect("the ClientRequest block is brace-terminated at column 0");
    let block = &rest[..end];
```
Pair it with the runtime wire proof (spike 008 step 1) — `serde_json::from_value::<ClientRequest>(json!({"method":"skills/list","params":{}}))` must be `Err`, with `resources/list` as the control that is `Ok`.

---

## Runtime State Inventory

Not applicable — this is an additive protocol-surface phase, not a rename/refactor/migration. There is no stored data, live service config, OS-registered state, secret, or build artifact carrying a string this phase changes.

The **one** state-shaped item is the retirement of `skill://index.json`, which is a *served* resource, not stored state. Its complete consumer inventory is in Pattern 3 above (14 in-repo assertion/doc sites). No external consumer is known in this repo; a downstream host that reads `skill://index.json` would break, which is why "legacy gate" is offered as an alternative to outright removal.

---

## Environment Availability

| Dependency | Required by | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| `cargo` / rustc | everything | ✓ | workspace resolves; `pmcp` at 2.19.3 in lock | — |
| `sha2` crate | digest computation | ✓ | 0.11.0 (and 0.10.9 also in lock, via other crates) | — |
| `serde_yaml` crate | frontmatter → JSON | ✓ | 0.9.34+deprecated, already in `Cargo.lock` | flat-frontmatter limit |
| `cargo audit` | `make audit` | ✓ | runs, exit 0, 7 allowed warnings | — |
| `gh` CLI | re-fetching the SEP draft | ✓ | `gh pr view 2640` succeeded | `curl` on raw.githubusercontent.com (also verified) |
| `pmat` | CI cognitive-complexity gate | not probed | pinned 3.15.0 in CI | CI-only per D-07; not needed locally |
| `mdbook` | `make book-build` / `book-test` | not probed | Makefile:1327 auto-installs when missing | — |
| Network to crates.io + raw.githubusercontent.com | version/spec verification | ✓ | — | — |

**Missing dependencies with no fallback:** none.

---

## State of the Art

| Old approach | Current approach | When changed | Impact |
|--------------|------------------|--------------|--------|
| Skills discovered via a synthesized `skill://index.json` resource (agentskills.io discovery schema 0.2.0) | `skills/list` RPC method | SEP-2640 rewrite, 2026-08-29 | Gap #3 — the index is nonstandard *and* violates the URI structure rules |
| No new RPC methods; pure resources mapping | Three methods: `skills/list` (MUST), `skills/get` (MUST), `resources/directory/read` (optional) | 2026-08-29 | Gaps #1, #6 |
| Entry metadata = a curated `{name, type, description, url}` | Verbatim frontmatter JSON + complete `{uri, digest, size}` manifest | 2026-08-29 | Gap #2 |
| Archive distribution (`application/gzip`) as an optional mode | **Formally dead** — moved to "Appendix: Deferred Features" with Core-Maintainer objections on record | 2026-08-29 | pmcp's v1 exclusion is vindicated. **Do not resurrect.** |
| `serde_yaml` as the default Rust YAML crate | Archived by dtolnay; `serde_yaml_ng` is the maintained fork; `serde_yml` is now itself deprecated | serde_yaml archived 2024; `serde_yml` deprecated by 2026-05-27 | The YAML decision is genuinely open — see Open Question 3 |

**Deprecated / outdated:**
- `skill://index.json` and `build_discovery_index_json` — retire or legacy-gate.
- `serde_yml` / `serde-yml` — self-declared unmaintained shim. Remove from any candidate list.
- Spike 008's `format!("sha256:{:x}", …)` snippet — written against `sha2` 0.10; this workspace is on 0.11.

---

## Assumptions Log

| # | Claim | Section | Risk if wrong |
|---|-------|---------|---------------|
| A1 | `sha2` 0.11 / `digest` 0.11 may no longer implement `LowerHex` on the finalize output, so `format!("{:x}", h.finalize())` may not compile | Pattern 2 | Low — a compile error caught in the first minute of Wave 0. Recorded because the spike's copy-pasteable snippet is 0.10-era. **Verify empirically before writing the digest fn.** |
| A2 | `serde_yaml::from_str::<serde_json::Value>(frontmatter)` round-trips typical SKILL.md frontmatter faithfully (nested maps, lists, scalars) | Standard Stack | Medium — YAML non-string keys and some scalar-typing edge cases (`yes`/`no`, `1.0`, sexagesimals) can diverge from what a host's YAML parser produces, which the draft makes a hard load failure. Needs a proptest/fixture pass. |
| A3 | `make comply` / `pmat comply check` has no existing `pmcp` skills contract that must be updated | Project Constraints | Low — check `../provable-contracts/contracts/pmcp/` before the first commit; a missing contract update fails `make quality-gate` at the last leg. |
| A4 | CRLF-authored SKILL.md frontmatter parses identically to LF under the chosen YAML path | Pitfall 4 / Validation | Medium — `tests/skills_integration.rs:61` (`build_widget_skill_crlf`) and `src/server/skills.rs:781` already lock CRLF behaviour for the *description* scanner. The new frontmatter extractor must match, or an existing test fails. |
| A5 | No downstream consumer outside this repo depends on `skill://index.json` | Runtime State Inventory | Low-Medium — pmcp is published to crates.io; a legacy gate rather than removal makes this assumption cost-free. |
| A6 | The `sha256:` digest should cover the same bytes `resources/read` returns (`skill.body()` as UTF-8), with `size = body.len()` | Pattern 2 | Low — this is the only interpretation consistent with SEP line 257 ("the length in bytes of the file's raw content — the same bytes the `digest` covers") and with the fact that the registry stores `String`, not `Vec<u8>`. |

---

## Open Questions (RESOLVED)

> All 7 questions below were resolved on 2026-09-01 — Q1→D-01, Q2→D-02, Q3→D-04,
> Q4→D-10 (deferral), Q5→CONTEXT Deferred Ideas, Q6→D-06, Q7→D-09. Authoritative
> record: 125-CONTEXT.md `<decisions>`. Retained verbatim below for the reasoning.

1. **Transport reach: does `skills/list` need to work over stdio?** *(RESOLVED → D-01: HTTP-only, recorded stdio deferral)*
   - *What we know:* the `InternalClientRequest` route reaches only streamable HTTP. Over stdio the frame fails at `parse_message` and the server actor breaks the loop (measured above). The seam's rustdoc explicitly says widening is a non-semver-breaking follow-on. `examples/s44`/`c10` do not use a transport at all — they call the `ResourceHandler` trait directly — so they will pass either way, which is itself a hazard.
   - *What's unclear:* whether HTTP-only reach satisfies "a pmcp server that declares the extension actually answers it" for this milestone.
   - *Recommendation:* **Treat this as the phase's keystone decision.** Widening `IngressRequest::Internal` into `run_transport_actor` is a bigger change than the skills work itself (the actor's `request_tx` channel is typed `(RequestId, Request)` — the public enum — so widening means changing that channel's type or adding a second one). A defensible middle path: land HTTP-only in this phase, and add a stdio-reach plan/task that is *explicitly deferred with a recorded owner*, never silently dropped (success criterion #5's discipline, applied to a gap the criteria do not name).

2. **What does a frontmatter-less skill do?** *(RESOLVED → D-02: warn + exclude)* Three candidate answers, all defensible, none free:
   - (a) **Error at `into_handler()`** — "a skill registered on a server declaring SEP-2640 must carry frontmatter." Cleanest conformance; breaks 30+ existing tests/doctests/proptests and every `Skill::new("x", "body")` in the book.
   - (b) **Exclude from `skills/list`, still serve via `resources/read`** — legal (SEP line 231: "MAY return an empty or partial listing"), but then `skills/get` on it must also error, and the skill is invisible.
   - (c) **Synthesize `{name, description}`** — guaranteed host-side verification failure (SEP line 269). **Not recommended.**
   - *Recommendation:* (b) for existing constructions plus a build-time **warning**, with (a) available behind a strict/`try_` variant. Confirm with the user — this changes observable behaviour for every skill in the repo's tests.

3. **YAML dependency: take `serde_yaml` 0.9 (deprecated but already in the graph) or a maintained alternative?** *(RESOLVED → D-04: serde_yaml 0.9, isolated)*
   - *What we know:* `serde_yaml` is already in `Cargo.lock`, already a production dep of four workspace crates, has no RUSTSEC advisory, and adding it costs zero new packages. `serde_yaml_ng` is the maintained fork but has not published since 2024-05-26 and *does* add a package. `serde_yml` is out. `saphyr` is alive but has no serde bridge.
   - *What's unclear:* whether the project has a policy against depending on an archived crate in *shipped* code (as opposed to dev/test code and non-core workspace crates).
   - *Recommendation:* `serde_yaml` 0.9, optional, gated on `skills`. Isolate the parse behind one crate-private fn so swapping it later is a one-file change.

4. **`resources/directory/read` (gap #6): defer, and say so where?** *(RESOLVED → D-10: defer, rustdoc note)* The current `{}` declaration already means `directoryRead: false` and is legal. Success criterion #5 requires the deferral be *explicit*. Recommendation: a rustdoc note on `set_skills_capabilities` plus a row in the phase's deferred-items record — **not** a code `TODO` (`make check-todos` is in the gate and CLAUDE.md forbids SATD).

5. **Client wrappers (gap #7): in scope?** *(RESOLVED → deferred, CONTEXT Deferred Ideas)* `client.list_skills()` / `get_skill()` / `read_skill_uri()`. TypeScript-SDK precedent exists. These are *additive public API* on `Client`, which compiles on wasm32 — same constraint that put `ServerDiscoverResult` in `types::protocol` rather than in the server (`src/types/protocol/mod.rs` rustdoc). If deferred, record it the same way as #6.

6. **`skills/get` on an unknown URI: `-32602` per the draft, or `-32601` to match the shipped `SkillsHandler::read`?** *(RESOLVED → D-06: -32602)* The draft says `-32602`. The shipped read handler says `-32601` (`src/server/skills.rs:556-559`). Recommendation: follow the draft for `skills/get`, and record the `resources/read` divergence as a separate, out-of-scope observation rather than fixing it here (changing an existing error code is observable behaviour with its own test, `resources_read_unknown_uri_method_not_found` at `tests/skills_integration.rs:253`).

7. **Does the phase add `skills` to `full`?** *(RESOLVED → D-09: no; dedicated make test-skills leg)* It would fix Pitfall 2 permanently, but `full` and `full-v2` are two enumerated lists whose drift is asserted by `tests/v1_severability_tripwire.rs` (which derives both from `Cargo.toml` at test time). Adding to `full` alone would change what the severance proof covers. Needs a decision; a dedicated `make test-skills` leg is the lower-risk alternative.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `#[tokio::test]` + `proptest` (dev-dep) + `quickcheck` (dev-dep) |
| Config file | none — `Cargo.toml` `[dev-dependencies]` + `Makefile` targets |
| Quick run command | `cargo test -p pmcp --all-features --lib skills -- --test-threads=1` |
| Full suite command | `cargo test --all-features -- --test-threads=1` (this is what CI runs, `.github/workflows/ci.yml:104`) |

> **Do NOT use `make test-unit` / `make test-integration` as the quick run** — they pin `--features "full"`, which excludes `skills`, and report success having run zero tests from this module. See Pitfall 2.
>
> **Do NOT use `cargo nextest -E 'test(/foo/)'`** — a project-recorded false-green (the selector silently matches zero tests and exits 0). Use `binary(<name>)` if nextest is used at all.

### Phase Requirements → Test Map

| Gap | Behavior | Test type | Automated command | File exists? |
|-----|----------|-----------|-------------------|--------------|
| #1a | `ClientRequest` gains no `skills/*` variant (semver) | unit (source scan) | `cargo test --all-features --test skills_routing client_request_has_no_skills_variants` | ❌ Wave 0 |
| #1b | `from_value::<ClientRequest>({"method":"skills/list"})` is `Err`; `resources/list` is `Ok` (control) | unit | `cargo test --all-features --test skills_routing wire_proof` | ❌ Wave 0 |
| #1c | `classify_internal_method("skills/list", …)` returns the new variant; `"skills/lists"` returns `None` | unit (in-module) | `cargo test -p pmcp --all-features --lib classify_internal_method` | ✅ pattern exists at `src/types/protocol/mod.rs:1066-1111` |
| #1d | A live server answers `skills/list` / `skills/get` over the wire | integration (HTTP) | `cargo test --all-features --test skills_routing served_` | ❌ Wave 0 (reuse `tests/common/v2` harness — `spawn_*_server`, `post`, `v2_headers`) |
| #1e | **Stdio reach** — whichever answer Open Question 1 takes, assert it | integration | `cargo test --all-features --test skills_routing stdio_reach` | ❌ Wave 0 |
| #2a | Entry carries verbatim frontmatter incl. non-required fields (`license`, nested `metadata`) | unit | `cargo test -p pmcp --all-features --lib entries_frontmatter_verbatim` | ❌ Wave 0 |
| #2b | Every digest matches `^sha256:[0-9a-f]{64}$`; every `size` equals the served bytes' length | property | `cargo test -p pmcp --all-features --lib prop_entry_digest_shape` | ❌ Wave 0 |
| #2c | `resources` manifest is complete and includes the entry's own `uri` first | unit | `cargo test -p pmcp --all-features --lib entries_manifest_complete` | ❌ Wave 0 |
| #2d | Fuzz: arbitrary bytes as SKILL.md body never panic entry synthesis | fuzz | `cargo fuzz run fuzz_skill_entry` (see `fuzz/`) | ❌ Wave 0 |
| #3 | `skill://index.json` absent from `resources/list` by default | unit + integration | `cargo test --all-features skills` | ⚠️ exists but **inverted** — 14 sites assert its presence (Pattern 3) |
| #4a | `with_path("acme/billing")` on a skill named `refunds` is rejected | unit | `cargo test -p pmcp --all-features --lib name_identity` | ❌ Wave 0 |
| #4c | Constructor name ≠ frontmatter name is rejected **when frontmatter exists** | unit | same | ❌ Wave 0 |
| #5 | >512 files or >16 MiB warns at `into_handler()` | unit | `cargo test -p pmcp --all-features --lib limits_warn` | ❌ Wave 0 |
| SC#4 | SKILL.md + supporting files still byte-identical; refs still absent from `resources/list`; dual-surface byte-equality holds (LF + CRLF) | integration + property | `cargo test --all-features --test skills_integration` | ✅ `tests/skills_integration.rs` (9 tests + 2 proptests) |
| SC#4 | s44 / c10 still pass | example | `cargo run --example s44_server_skills --features skills,full` and `--example c10_client_skills` | ✅ exist; c10 **asserts** on index.json and will need editing |

### Sampling Rate

- **Per task commit:** `cargo test -p pmcp --all-features --lib skills -- --test-threads=1` (seconds; catches the module's own regressions)
- **Per wave merge:** `cargo test --all-features -- --test-threads=1` (matches CI exactly)
- **Phase gate:** `make quality-gate` **AND** `cargo test --all-features -- --test-threads=1` **AND** `cargo clippy --all-targets --all-features -- -D warnings` (the gate's own `lint` leg does not reach this module) — then `/gsd-verify-work`

> `--test-threads=1` is not optional: CLAUDE.md mandates it, and the project has recorded parallel-test races elsewhere in the workspace.

### Wave 0 Gaps

- [ ] `tests/skills_routing.rs` — the new integration test file; header `#![cfg(all(feature = "skills", not(target_arch = "wasm32")))]` plus whatever transport features the chosen reach requires (`streamable-http`, `http-client` if HTTP-only — mirror `tests/v2_tasks_update_routing.rs:53-57`)
- [ ] Verify `sha2` 0.11 hex-formatting API before writing the digest fn (A1)
- [ ] Verify `serde_yaml::from_str::<serde_json::Value>` on an LF and a CRLF frontmatter fixture (A2, A4)
- [ ] Check `../provable-contracts/contracts/pmcp/` for an existing skills contract (A3)
- [ ] Decide and record Open Questions 1, 2, 3 **before** any code plan
- [ ] Baseline run: `cargo test --all-features -- --test-threads=1` on a clean tree, so the phase's failures are distinguishable from pre-existing ones
- [ ] `fuzz/` target registration for entry synthesis (CLAUDE.md ALWAYS requirement)

---

## Security Domain

`security_enforcement` is not set in `.planning/config.json`, so it is **enabled**.

### Applicable ASVS Categories

| ASVS category | Applies | Standard control |
|---------------|---------|------------------|
| V2 Authentication | no | This phase adds no auth surface. But see the ordering note below. |
| V3 Session Management | no | The two methods are stateless reads; `HttpIngress::is_initialize` must return `false` for them so they never mint a session (`src/server/streamable_http_server.rs:2242-2255`). |
| V4 Access Control | **yes** | `skills/list` discloses the full skill catalog — names, descriptions and every file URI. If the server's resources are authorization-filtered, the entry projection must respect the same filter. The `ServerDiscoverResult::cache_scope` rustdoc records exactly this class of concern for a capability projection ("sharing it across authorization contexts would disclose capabilities one caller may not hold"). |
| V5 Input Validation | **yes** | `skills/get`'s `uri` param is attacker-controlled. Answer from the registry's exact-match map only — never by string-manipulating the URI into a path. The existing `validate_reference_path` (`src/server/skills.rs:321-357`) already rejects `..`, leading `/`, `://` and null bytes at registration; the lookup side must not re-open what registration closed. |
| V6 Cryptography | **yes** | SHA-256 only, via `sha2`. Never hand-roll. **The draft is explicit (line 267): "Digests are unsigned and supplied by the same server that supplies the content … Hosts MUST NOT treat a digest match as a security boundary."** Do not document pmcp's digests as an integrity guarantee. |
| V7 Error Handling / Logging | **yes** | `skills/get` on an unknown URI returns `-32602` with a message. Do not echo the caller's raw URI into a log line without bounding it. |
| V8 Data Protection | partial | The `frontmatter` object is emitted verbatim, including any author-supplied field. A server author who puts a secret in SKILL.md frontmatter now leaks it to every caller of `skills/list`, where before it required a `resources/read`. Worth a rustdoc warning. |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard mitigation |
|---------|--------|---------------------|
| Path traversal via `skills/get` `uri` | Tampering / Information disclosure | Exact-match lookup in the `IndexMap`; no path joining. Registration-time `validate_reference_path` already blocks `..`. |
| Catalog disclosure across authorization contexts | Information disclosure | Project entries per-request from the same source the resource surface is filtered by; do not cache one caller's projection for another. Do not set a shared `cacheScope` on an authorization-filtered listing. |
| Unbounded response (memory/DoS) via a huge registry | Denial of service | The 512-file / 16 MiB per-skill guard (gap #5); pagination via `cursor`/`nextCursor` for the catalog itself. |
| Params parse-error ordering inversion | Elevation of privilege | Follow the `TasksUpdate` discipline (`src/types/protocol/mod.rs:798-806`): the classifier must **never** reject a body. A malformed `skills/get` params becomes a `-32602` in the served branch, **after** any auth gate, not a parse error before it. Deserializing in the classifier would hand an unauthenticated caller a params error instead of an auth refusal. |
| Digest treated as trust | Spoofing | Explicit rustdoc: the digest binds content to *this* listing, not to any authority. |
| Malicious skill content reaching an agent | Elevation of privilege | Out of scope here (host-side), but the draft's §Security Implications applies to `pmcp-agent` when it consumes skills — flagged by spike 008 for spike 010's successor work. |

---

## Sources

### Primary (HIGH confidence)
- `src/types/protocol/mod.rs` — `ServerDiscoverRequest` rustdoc (:583-600), `InternalClientRequest` (:760-816), `SERVER_DISCOVER_METHOD` / `TASKS_UPDATE_METHOD`, `classify_internal_method` (:873-886), its tests (:1066-1111). Read this session.
- `src/shared/protocol_helpers.rs` — `IngressRequest` (:15-55), `parse_request_or_internal` (:57-100), `parse_request` (:110-116). Read this session.
- `src/shared/transport.rs` — `parse_message` (:115), `parse_method_message` (:130-150). Read this session.
- `src/server/mod.rs` — module gate (:188-202), `ServerBuilder` skills fields/methods (:3152-3156, :3258-3259, :4472-4565), `handle_discover` (:1650-1675), `run_transport_actor` (:1437-1478), `route_inbound_message` (:1483-1504), skills finalization (:5365-5374). Read this session.
- `src/server/skills.rs` (1394 lines) — read in full through :700 and grepped throughout. All constants, `Skill`/`SkillReference`/`Skills`, `SkillsHandler`, `build_discovery_index_json`, `SkillPromptHandler`, `ComposedResources`, `parse_frontmatter_description`, and the unit-test/proptest block.
- `src/server/builder.rs` — `pending_skills` (:138-142), builder methods (:433-525), `finalize_skills_resources` (:1426-1455). Read this session.
- `src/server/core.rs` — `ServerCore` struct (:452-512), `request_is_cacheable` + its `server/discover` rustdoc (:2130-2200), `build_discover_response` (:2380), discover tests (:5234-5330). Read this session.
- `src/server/streamable_http_server.rs` — `HttpIngress` variants + `is_initialize` (:2100-2255), `classify_http_ingress` (:2260-2318), response-assembly sites (:3243, :3785, :4940, :5022, :5135). Read this session.
- `Cargo.toml` — features (:300-316), `sha2` (:149), `serde_yaml` dev-dep (:249), example declarations (:692-699). Read this session.
- `Cargo.lock` — `serde_yaml 0.9.34+deprecated` (:6654-6664), `sha2 0.11.0` / `0.10.9`, `digest 0.11.2` / `0.10.7`. Read this session.
- `Makefile` — `quality-gate`, `lint`, `test-all`, `test-unit`, `test-doc`, `test-property`, `test-examples`, `test-integration`, `build`, `doc-check`, `audit`, `unused-deps`, `purity-check`, `no-crypto-check`, `PURITY_*` (:1401-1402, :1570). Read this session.
- `.github/workflows/ci.yml` — feature sets at :63, :90, :101, :104, :113, :164, :174, :348, :460. Grepped this session; zero `skills` matches.
- `tests/skills_integration.rs` — header cfg (:27), all test fns, index assertions (:168-188, :225, :351-380). Read this session.
- `tests/v2_tasks_update_routing.rs` — the semver tripwire (:1196-1208) and the file's cfg header (:53-57). Read this session.
- `examples/s44_server_skills.rs`, `examples/c10_client_skills.rs`, `examples/skills/*/SKILL.md`. Read this session.
- **SEP-2640 draft**, PR #2640 head branch `sep/skills-extension`, `seps/2640-skills-extension.md`, 644 lines. Fetched this session from `raw.githubusercontent.com`. `gh pr view 2640` confirms `state: OPEN`, `headRefName: sep/skills-extension`, `updatedAt: 2026-08-29T18:46:46Z` — **unchanged since spike 008 ran**, so the spike's capture is still current.
- **Empirical probe** — scratch binary at `<scratchpad>/probe/`, `pmcp` path dep with `default-features = false, features = ["skills"]`, output pasted in Pitfall 1.
- `cargo audit` — run this session, exit 0.
- crates.io API (`/api/v1/crates/{serde_yaml,serde_yaml_ng,serde_yml,saphyr,saphyr-serde,yaml-rust2,noyalib}`) — fetched this session.
- `gsd-tools query package-legitimacy check --ecosystem crates …` — run this session.

### Secondary (MEDIUM confidence)
- `.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md` — the fix blueprint (its `serde-yml` candidate is now refuted; its `sha2` snippet is 0.10-era).
- `.planning/spikes/008-sep-2640-drift-check/{README.md,src/main.rs,Cargo.toml}` — the measured drift and the wire-proof technique.
- `.planning/spikes/CONVENTIONS.md:155-167` — the wire-proof convention.
- `.planning/ROADMAP.md` — the Phase 125 section.
- `CLAUDE.md` — quality gates, ALWAYS requirements, contract-first, release/semver policy.

### Tertiary (LOW confidence)
- None. Every claim above was either read from source this session, fetched from the PR head branch this session, or measured by running a command this session.

---

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — `sha2` already present (manifest read); YAML candidates verified against the crates.io API this session, with one spike recommendation actively refuted.
- Architecture: **HIGH** — every routing hop read from source with line citations; the transport-reach limit confirmed both by the seam's own rustdoc and by an empirical probe.
- Pitfalls: **HIGH** — each pitfall is backed by a measured count (14 index sites, 40+ `Skill::new` sites, per-Makefile-leg feature audit) or a pasted probe output.
- SEP wire shape: **HIGH** — re-fetched from the PR head branch this session; `resultType` and the `-32602` error code are two details the spike's capture did not record.
- Open questions: these are genuine decisions, not gaps in the research. Route them through `/gsd-discuss-phase 125`.

**Research date:** 2026-09-01
**Valid until:** 2026-09-15 (14 days) — SEP-2640 is an in-review draft that was rewritten once already during this milestone's lifetime. **Re-run `gh pr view 2640 --json headRefName,updatedAt` before planning locks**; if `updatedAt` has moved past `2026-08-29T18:46:46Z`, re-fetch the raw markdown and re-check §Enumeration, §Retrieval and §Capability Declaration.

### Plans to Review

---

---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - Cargo.toml
  - src/types/protocol/mod.rs
  - src/server/streamable_http_server.rs
  - src/server/skills.rs
  - src/server/core.rs
  - src/server/mod.rs
  - src/server/builder.rs
  - tests/skills_routing.rs
autonomous: true
requirements: [D-01, D-04, D-05, D-07, D-11]
user_setup: []

estimate:
  tokens: 60000
  raw_tokens: 120000
  tasks: 2
  confidence: high

must_haves:
  truths:
    - "A built `Server` carrying one frontmatter-bearing skill answers a live `skills/list` POST over streamable HTTP with a `result.skills` array whose single entry carries `uri`, verbatim `frontmatter` JSON, and a `resources` manifest entry matching `^sha256:[0-9a-f]{64}$` with a `size` equal to the served byte length (D-05, ROADMAP SC#1)."
    - "`serde_json::from_value::<ClientRequest>` on a `skills/list` or `skills/get` frame still returns `Err`, while the same call on `resources/list` returns `Ok` — the 2.x exhaustive-enum promise holds (ROADMAP SC#1)."
    - "`skills/list` answers on a v1 connection as well as a v2 one — unlike `server/discover` it carries NO era gate (D-07: only the `ttlMs`/`cacheScope` attributes are 2026-07-28-conditional)."
    - "The `skills/list` result carries `resultType: \"complete\"`, contains every registered skill in one response, and emits no `nextCursor` key (D-07, D-11)."
    - "`skills/list` and `skills/get` are NOT name-bearing methods: `pmcp::testing::routing_name_key` yields nothing for them, and a v2 POST carrying an empty `Mcp-Name` is accepted."
    - "The YAML frontmatter parse is reachable through exactly one crate-private function, so the YAML crate can be swapped in a single file (D-04)."
    - "Transport reach is streamable HTTP only, and a test asserts the recorded stdio behavior rather than leaving it unmeasured (D-01)."
  artifacts:
    - Cargo.toml
    - src/types/protocol/mod.rs
    - src/server/streamable_http_server.rs
    - src/server/skills.rs
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/builder.rs
    - tests/skills_routing.rs
  key_links:
    - "`classify_internal_method` (src/types/protocol/mod.rs) -> `parse_request_or_internal` (src/shared/protocol_helpers.rs) -> `classify_http_ingress` (src/server/streamable_http_server.rs): the single interception point. A method spelling present in one and absent in the other is a silent no-route."
    - "`classify_http_ingress` inner `match` over `InternalClientRequest` is deliberately exhaustive — adding the new variants is a compile-time tripwire that forces every HttpIngress site to be written."
    - "`finalize_skills_resources` (src/server/builder.rs) -> `Server.skill_entries` (src/server/mod.rs): the ONE place both build paths get entries from. Reaching entries by downcasting the ResourceHandler breaks the day `ComposedResources` wraps it."
    - "`Skills::entries()` (public, `&self`) must be called BEFORE `Skills::into_handler()` (public, consumes `self`) — `into_handler`'s return type is public API and MUST NOT change to a tuple."
---

<objective>
Land the tracer: one frontmatter-bearing skill, registered on a `Server`, answered
end-to-end on a live `skills/list` POST over streamable HTTP with a conforming
entry whose digest is verified on the wire.

Purpose: prove the whole architecture — crate-private classifier route, entry
synthesis at build time, entries carried as their own field, shared projection in
`core.rs`, thin delegate in `mod.rs`, wire assembly in the HTTP transport — on one
path before any expansion task builds out from it. If the `InternalClientRequest`
route cannot carry `skills/list`, this is the commit that says so.

Output: a working `skills/list` over HTTP, a public `Skills::entries()` /
`SkillEntry` API, and `tests/skills_routing.rs` carrying both the live-wire proof
and the routing-guarantee proofs.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-CONTEXT.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-RESEARCH.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-PATTERNS.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-VALIDATION.md
@.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md
</context>

## Artifacts this phase produces

Symbols and files **created by this phase** (all five plans). Newly-created symbols
are not drift candidates — they do not exist upstream of this phase.

**Created in this plan (125-01):**

| Symbol / artifact | Kind | Location | Visibility |
|---|---|---|---|
| `SKILLS_LIST_METHOD` | const `&str` = `"skills/list"` | `src/types/protocol/mod.rs` | `pub(crate)` |
| `SKILLS_GET_METHOD` | const `&str` = `"skills/get"` | `src/types/protocol/mod.rs` | `pub(crate)` |
| `InternalClientRequest::SkillsList` | enum variant (carries raw `params`) | `src/types/protocol/mod.rs` | `pub(crate)` |
| `HttpIngress::SkillsList` | enum variant `{ id, params }` | `src/server/streamable_http_server.rs` | private |
| `SkillEntry` | `#[non_exhaustive]` struct + accessors | `src/server/skills.rs` | `pub` |
| `SkillResourceRef` | `#[non_exhaustive]` struct (`uri`/`digest`/`size`) | `src/server/skills.rs` | `pub` |
| `Skills::entries()` | `pub fn entries(&self) -> Result<Vec<SkillEntry>>` | `src/server/skills.rs` | `pub` |
| `parse_frontmatter_value` | crate-private YAML->JSON fn (the D-04 isolation point) | `src/server/skills.rs` | private |
| `sha256_digest_hex` | crate-private digest formatter | `src/server/skills.rs` | private |
| `build_skills_list_response` | shared projection free fn | `src/server/core.rs` | `pub(crate)` |
| `Server::handle_skills_list` | thin delegate | `src/server/mod.rs` | `pub(crate)` |
| `Server.skill_entries` | struct field `Arc<IndexMap<String, SkillEntry>>` | `src/server/mod.rs` | private |
| `assemble_skills_list_fast` / `assemble_skills_list_with_middleware` | HTTP response assemblers | `src/server/streamable_http_server.rs` | private |
| `tests/skills_routing.rs` | NEW integration test file | repo root `tests/` | test crate |
| `serde_yaml` optional dep + `skills = ["dep:serde_yaml"]` | Cargo feature wiring | `Cargo.toml` | — |

**Created in later plans of this phase:** `InternalClientRequest::SkillsGet`,
`HttpIngress::SkillsGet`, `build_skills_get_response`, `Server::handle_skills_get`,
`ServerCore.skill_entries` + its two delegates (125-02); the manifest-completeness,
warn+exclude, name-identity and limits validation paths in `Skills::entries()`
(125-03); `make test-skills` Makefile target, `fuzz/fuzz_targets/fuzz_skill_entry.rs`
and its `[[bin]]` registration (125-05).

**Retired in 125-04:** `SKILL_INDEX_URI`, `build_discovery_index_json`, and the
`index_json` field on `SkillsHandler`.

## Source Coverage Audit (phase-wide)

| SOURCE | ID | Feature / Requirement | Plan | Status |
|---|---|---|---|---|
| GOAL | — | A pmcp server that declares `io.modelcontextprotocol/skills` actually answers it | 01,02,03 | COVERED |
| GOAL | SC#1 | No new public `ClientRequest` variant, yet a built server answers `skills/list`+`skills/get` with verbatim frontmatter + complete `{uri,digest,size}` manifests | 01,02,03 | COVERED |
| GOAL | SC#2 | `skill://index.json` no longer served by default | 04 | COVERED |
| GOAL | SC#3 | Build-time name-identity rejection + >512-file / >16 MiB warning | 03 | COVERED |
| GOAL | SC#4 | Existing conforming behavior unchanged (byte-identical reads, refs unlisted, dual-surface byte equality, s44/c10 pass) | 03,04 | COVERED |
| GOAL | SC#5 | `resources/directory/read` and client wrappers explicitly deferred, never silently dropped | 05 | COVERED |
| REQ | — | (none — ROADMAP records `**Requirements**: TBD`; CONTEXT decisions are the tracked set) | — | N/A |
| RESEARCH | Gap #1 | `skills/list` + `skills/get` routed via the classifier | 01,02 | COVERED |
| RESEARCH | Gap #2 | Entry-manifest API (frontmatter, digest, size) | 01,03 | COVERED |
| RESEARCH | Gap #3 | `skill://index.json` retirement + 14-site blast radius | 04 | COVERED |
| RESEARCH | Gap #4 | Name-identity validation (4a unconditional, 4c frontmatter-conditional) | 03 | COVERED |
| RESEARCH | Gap #5 | 512-file / 16 MiB limits guard | 03 | COVERED |
| RESEARCH | Gap #6 | `resources/directory/read` — deferred with a record | 05 | COVERED (deferral) |
| RESEARCH | Gap #7 | Client wrappers — deferred with a record | 05 | COVERED (deferral) |
| RESEARCH | Pitfall 1 | Transport reach measured + asserted, stdio deferral recorded | 01,05 | COVERED |
| RESEARCH | Pitfall 2 | Gate blind spot — dedicated `make test-skills` leg | 05 | COVERED |
| RESEARCH | Pitfall 3 | 40+ frontmatter-less call sites keep working | 03 | COVERED |
| RESEARCH | Pitfall 4 | Frontmatter-less skill never silently synthesized | 03 | COVERED |
| RESEARCH | Pitfall 5 | `resultType` / `ttlMs` / `cacheScope`; no `request_is_cacheable` row | 01,02 | COVERED |
| RESEARCH | Pitfall 6 | Two build paths both carry entries | 01,02 | COVERED |
| RESEARCH | A1 | `sha2` 0.11 has no `LowerHex` — resolved at plan time, see 125-01 T1 | 01 | COVERED |
| RESEARCH | A2/A4 | LF + CRLF frontmatter fixtures | 01,03 | COVERED |
| RESEARCH | A3 | Contract check — resolved at plan time (no pmcp skills contract exists) | 05 | COVERED |
| CONTEXT | D-01 | HTTP-only reach; stdio a recorded deferral, not a code TODO | 01,05 | COVERED |
| CONTEXT | D-02 | Warn + exclude for frontmatter-less skills | 03 | COVERED |
| CONTEXT | D-03 | Cleanup scope: canonical surfaces only | 04 | COVERED |
| CONTEXT | D-04 | `serde_yaml` 0.9 optional, gated on `skills`, isolated behind one fn | 01 | COVERED |
| CONTEXT | D-05 | `sha2` 0.11 for `sha256:{64 lowercase hex}` | 01,03 | COVERED |
| CONTEXT | D-06 | `skills/get` unknown URI returns `-32602` | 02 | COVERED |
| CONTEXT | D-07 | `resultType: complete`; cacheability named at the projection call site | 01,02 | COVERED |
| CONTEXT | D-08 | `skill://index.json` retires | 04 | COVERED |
| CONTEXT | D-09 | Dedicated `make test-skills` leg in quality-gate | 05 | COVERED |
| CONTEXT | D-10 | Keep auto-declaring; rustdoc the HTTP-only reach + `directoryRead: false` | 05 | COVERED |
| CONTEXT | D-11 | Single page, no `nextCursor` | 01,05 | COVERED |

All rows COVERED. CONTEXT `## Deferred Ideas` (stdio reach, `resources/directory/read`,
client wrappers, strict frontmatter mode, the `resources/read` `-32601` fix) are
excluded by rule and appear in no plan as implementation work — only as recorded
deferrals in 125-05.

## Plan-time findings that supersede RESEARCH

Three facts were measured while writing this plan. They are recorded here because
they change instructions the executor would otherwise follow.

1. **RESEARCH assumption A1 is RESOLVED, and the answer is the pessimistic one.**
   `grep -rln LowerHex` over `~/.cargo/registry/src/*/sha2-0.11.0/`,
   `digest-0.11.2/` and `crypto-common-0.2.2/` returns **zero files**. There is no
   `LowerHex` impl anywhere in the shipped `sha2` 0.11 stack, so
   `format!("{:x}", hasher.finalize())` will not compile. Format the finalized
   bytes with a `{:02x}` fold. This is no longer a Wave-0 probe.

2. **`Skills::into_handler` is PUBLIC and returns `Result<Arc<dyn ResourceHandler>>`
   (`src/server/skills.rs:437`).** Changing it to return a tuple, as
   125-PATTERNS.md suggests for `finalize_skills_resources`, would be a
   semver-MAJOR break on a `pub` method. Add a separate `pub fn entries(&self)`
   taking `&self`, and have the crate-private `finalize_skills_resources` call
   `entries()` first and `into_handler()` second.

3. **The index-retirement blast radius is 2 tracked sites LARGER than RESEARCH's
   table.** `src/server/builder.rs:2302` and
   `pmcp-course/src/quizzes/ch23-skills.toml:40` both assert/describe the index and
   are absent from RESEARCH Pattern 3. Conversely `pmcp-book/book/**` and
   `pmcp-course/book/**` are **untracked mdBook output** (`git ls-files` returns
   nothing for them) and must NOT be hand-edited. Carried into 125-04.

<tasks>

<task type="tracer" tdd="true">
  <name>Task 1: End-to-end "a host asks skills/list and gets a conforming entry" — one skill, one path</name>

  <files>Cargo.toml, src/types/protocol/mod.rs, src/server/streamable_http_server.rs, src/server/skills.rs, src/server/core.rs, src/server/mod.rs, src/server/builder.rs, tests/skills_routing.rs</files>

  <read_first>
    - src/types/protocol/mod.rs — read the `InternalClientRequest` enum (~:760-816), the `SERVER_DISCOVER_METHOD` / `TASKS_UPDATE_METHOD` constant block (~:820-840), `classify_internal_method` (~:871-886), and its in-module tests (~:1066-1111). The `TasksUpdate` variant is the exact shape to copy.
    - src/shared/protocol_helpers.rs — `IngressRequest` (:15-55) and `parse_request_or_internal` (:57-100). Read the rustdoc at :32-42 stating that `classify_http_ingress` is the ONLY production consumer; that sentence is the D-01 constraint.
    - src/server/streamable_http_server.rs — the `HttpIngress` variant block ending ~:2235, `is_initialize` (~:2237-2257), `classify_http_ingress` (~:2259-2313), the v2 header-gate arm at ~:3240-3250, `InternalResponseShape` (:3747), `assemble_discover_response_fast` (:3769), `assemble_tasks_update_fast` (:3888), `assemble_tasks_update_with_middleware` (:3942), `assemble_discover_response_with_middleware` (:4923), and the four per-path dispatch arms at :5022, :5068, :5135, :5164.
    - src/server/skills.rs — the module header doctest (:1-40), `Skill` struct + accessors (:155-320), `Skills::into_handler` (:437-471), `SkillsHandler::new` (:490-512), and `parse_frontmatter_description` (:656-680) for its BOM and CRLF handling.
    - src/server/core.rs — `build_discover_response` (~:2380-2430), the `request_is_cacheable` rustdoc (~:2153-2200), and the twin-site parity note (~:2434-2440).
    - src/server/mod.rs — `Server` struct fields (:469-500), `Server::handle_discover` (~:1647-1675), `handle_tasks_update` immediately below it, and the skills finalization site (~:5365-5380).
    - src/server/builder.rs — `finalize_skills_resources` (:1426-1455) and its `#[cfg]` / `#[cfg(not)]` call-site pair (:1356-1360).
    - src/server/request_state.rs:220-230 — the in-repo `Sha256::new()` / `update` / `finalize` idiom.
    - tests/v2_tasks_update_routing.rs — the header cfg block (:53-57), the once-restated method-literal convention (:74-76), and the module-doc property table (:26-35).
    - tests/common/v2.rs — `spawn_default_config` (:385), `v2_body` (:565), `v2_headers_for` (:752), `post` (:898), `Resp` (:809). Use these; do NOT add a new spawner.
    - Cargo.toml — the `skills = []` feature line (~:306), `sha2 = "0.11"` (:149), and the `serde_yaml = "0.9"  # Consumer: ...` dev-dep line (:249) for the local dep-justification comment convention.
  </read_first>

  <behavior>
    - `classify_internal_method("skills/list", &params)` returns `Some(InternalClientRequest::SkillsList { params })` with `params` passed through undecoded, including when params is `Value::Null`.
    - `classify_internal_method("skills/lists", ...)`, `("skills/", ...)` and `("skills", ...)` all return `None`.
    - A `Skills` registry holding one skill whose body opens with a `---` YAML frontmatter block containing `name`, `description` and a non-required field yields exactly one `SkillEntry` from `Skills::entries()`, whose `frontmatter` is a JSON object carrying all three keys with the authored values, and whose `resources` manifest's first element is the skill's own SKILL.md URI with a `sha256:`-prefixed 64-lowercase-hex digest and a `size` equal to the SKILL.md body's byte length.
    - A live `StreamableHttpServer` built from a `Server` carrying that registry answers a v2 `skills/list` POST at HTTP 200 with `result.resultType == "complete"`, `result.skills` of length 1, and no `nextCursor` key anywhere in `result`.
    - The same server answers a v1-framed `skills/list` POST without `-32601` — `skills/list` carries no era gate (unlike `server/discover`).
    - `HttpIngress::SkillsList` reports `is_initialize() == false`.
  </behavior>

  <action>
Wire ONE entry point — a `skills/list` POST — through every layer to the far end of
the stack, for a single registered skill. No `skills/get`, no `ServerCore` path, no
manifest expansion beyond the skill's own SKILL.md: those are expansion tasks. Real
error handling on this one path.

1. `Cargo.toml`: add `serde_yaml = { version = "0.9", optional = true }` to
   `[dependencies]` with a `# Consumer:` comment naming `src/server/skills.rs`
   frontmatter extraction, matching the convention on the existing dev-dep line.
   Change the feature to `skills = ["dep:serde_yaml"]`. Per D-09 do NOT add
   `skills` to `full` or `full-v2` — both are enumerated lists whose drift is
   asserted by `tests/v1_severability_tripwire.rs`.

2. `src/types/protocol/mod.rs`: add `SKILLS_LIST_METHOD` and `SKILLS_GET_METHOD`
   `pub(crate) const &str` constants with the values `skills/list` and `skills/get`
   in the same block as `SERVER_DISCOVER_METHOD`, carrying the same single-sourcing
   rationale rustdoc. Before minting them, grep the crate for either literal — a
   second constant with the same value is exactly the failure that rustdoc exists
   to prevent. Add `InternalClientRequest::SkillsList { params: serde_json::Value }`
   copying the `TasksUpdate` variant's shape and its two rustdoc sections
   ("# Why it is here and NOT a `ClientRequest` variant" and the raw-params
   rationale). Add the classifier arm returning `SkillsList` with `params.clone()`.
   Do NOT add `SkillsGet` yet — 125-02 owns it. Add the in-module classifier test
   with `skills/lists` and `skills/` as near-miss controls.

3. `src/server/skills.rs`: add three items.
   (a) `parse_frontmatter_value(body: &str) -> Option<serde_json::Value>` — the ONE
   crate-private function wrapping `serde_yaml::from_str::<serde_json::Value>`, per
   D-04. It strips a leading `\u{FEFF}` BOM and locates the block between the first
   two `---` delimiter lines exactly as `parse_frontmatter_description` does, so LF
   and CRLF bodies behave identically (RESEARCH A4; `tests/skills_integration.rs:61`
   and `src/server/skills.rs:781` already lock CRLF for the description scanner).
   Returns `None` when no frontmatter block is present or the YAML does not parse to
   a JSON object.
   (b) `sha256_digest_hex(bytes: &[u8]) -> String` — `Sha256::new()` / `update` /
   `finalize`, then fold the finalized bytes into a lowercase hex string with the
   `{:02x}` width-2 formatter and prefix `sha256:`. The `{:x}` whole-value formatter
   does NOT compile on this workspace's `sha2` 0.11 stack: there is no `LowerHex`
   impl in `sha2-0.11.0`, `digest-0.11.2` or `crypto-common-0.2.2` (measured at plan
   time; supersedes RESEARCH assumption A1 and the spike's 0.10-era snippet).
   (c) `SkillEntry` and `SkillResourceRef` public structs, both `#[non_exhaustive]`
   with private fields plus accessors, deriving `Clone`, `Debug` and `Serialize`.
   `#[non_exhaustive]` is required so a later field addition stays semver-MINOR.
   `SkillEntry` carries `uri: String`, `frontmatter: serde_json::Value`, and
   `resources: Vec<SkillResourceRef>`; `SkillResourceRef` carries `uri`, `digest`,
   `size`. Serialization must emit exactly the keys `uri`, `frontmatter`,
   `resources` and `uri`, `digest`, `size`.
   (d) `pub fn entries(&self) -> Result<Vec<SkillEntry>>` on `Skills`, taking
   `&self` — NOT changing `into_handler`'s signature, which is public API returning
   `Result<Arc<dyn ResourceHandler>>` and whose return type is frozen under the 2.x
   promise. For this tracer, synthesize one entry per skill whose body yields a
   frontmatter object, with a single-element `resources` manifest holding the
   skill's own `skill_md_uri()`, the digest over `skill.body().as_bytes()`, and
   `size = skill.body().len()`. Skills with no frontmatter are skipped silently for
   now; 125-03 adds the D-02 warning and the reference-file manifest entries.
   Add rustdoc with a doctest on `entries()` (CLAUDE.md requires rustdoc + doctest
   on new public API) and a warning that `frontmatter` is emitted verbatim, so any
   author-supplied secret in a SKILL.md frontmatter block is disclosed to every
   caller (ASVS V8).

4. `src/server/builder.rs`: change the crate-private `finalize_skills_resources` to
   return `(Option<Arc<dyn ResourceHandler>>, Vec<SkillEntry>)`, calling
   `skills.entries()` BEFORE `skills.into_handler()` since the latter consumes
   `self`. Update BOTH `#[cfg]` call sites — the `skills`-enabled arm and the
   `#[cfg(not(...))]` arm, which must now produce an empty entry vector or the
   non-skills build breaks.

5. `src/server/mod.rs`: add a private `skill_entries: Arc<IndexMap<String, SkillEntry>>`
   field on `Server`, keyed by SKILL.md URI, populated at the finalization site from
   the new tuple. Carry it as its OWN field — never reach it by downcasting the
   `ResourceHandler`, which `ComposedResources` may wrap. Add
   `pub(crate) fn handle_skills_list` as a THIN delegate with zero logic that calls
   the shared `core.rs` projection, mirroring `handle_discover`, and reuse
   `handle_tasks_update`'s "It defines no gate of its own" rustdoc sentence.
   Preserve the existing cfg asymmetry: `pub mod skills` is
   `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` while the
   `ServerBuilder` skills methods are plain `#[cfg(feature = "skills")]`. Do not
   harmonize them in this phase.

6. `src/server/core.rs`: add `pub(crate) fn build_skills_list_response`, modelled on
   `build_discover_response` with four deliberate deltas. Delete the era gate — do
   not translate it; `skills/list` has no version gate in the draft. Pass
   `ResponseDisposition::Complete`, which is what emits `resultType: "complete"`
   (D-07). Name `Cacheable::Yes` at this call site, exactly as
   `build_discover_response` does, and carry the comment explaining that this is why
   `request_is_cacheable` gets no row — its `match` has no wildcard arm and its
   rustdoc calls such a row a lie about where the claim is made. Emit the result as
   `{"skills": [...]}` with all entries in one response and NO `nextCursor` key
   (D-11) — record the cursor-pagination deferral in the rustdoc, not as a code
   comment marker.

7. `src/server/streamable_http_server.rs`: add `HttpIngress::SkillsList { id, params }`
   and handle it at all five required sites. (i) Variant declaration with rustdoc
   copying the `TasksUpdate` block's reasoning. (ii) `is_initialize` — add it to the
   `false` alternation; a skills method must never mint a session (ASVS V3).
   (iii) `classify_http_ingress` — extend the fast-reject condition to also let
   `SKILLS_LIST_METHOD` through, reading the single-sourced constant and never
   re-typing the literal, then add the inner `match` arm. Omitting the fast-reject
   extension is a silent no-route bug; omitting the inner arm is a compile error.
   (iv) The v2 header-gate arm — join the request-shaped alternation, and do NOT set
   a `method_override` (that is `server/discover`-specific because its method is
   pinned by classification). (v) The TWO per-path response-assembly arms, fast path
   and middleware path, via new `assemble_skills_list_fast` and
   `assemble_skills_list_with_middleware` functions reusing the existing
   `InternalResponseShape` struct. Both must exist or the two POST paths diverge.
   Also update the in-file classification tests near the `HttpIngress::TasksUpdate`
   sites so the new variant is covered.

8. `tests/skills_routing.rs` (NEW): header
   `#![cfg(all(feature = "skills", feature = "streamable-http", feature = "http-client", not(target_arch = "wasm32")))]`
   followed by `mod common;`, mirroring `tests/v2_tasks_update_routing.rs:53-57`.
   Open with the analog's numbered module-doc property table. Restate the two method
   literals once as file-level consts with the analog's justification comment (an
   integration crate cannot reach a `pub(crate)` constant). Use
   `common::v2::spawn_default_config` with a `Server::builder()` carrying one
   frontmatter-bearing skill; do NOT add a spawner to `tests/common/v2.rs`.
   Write the live-wire test: POST a v2 `skills/list` body built with
   `v2_body` and headers from `v2_headers_for`, assert HTTP 200,
   `result.resultType == "complete"`, `result.skills` length 1, entry `uri` equals
   the registered `skill://.../SKILL.md`, `frontmatter` carries the authored `name`,
   `description` and the non-required field, `resources[0].digest` matches
   `^sha256:[0-9a-f]{64}$`, `resources[0].size` equals the SKILL.md byte length, and
   `result` has no `nextCursor` key. Add a v1-framed twin asserting the response is
   not `-32601`, since `skills/list` has no era gate.
  </action>

  <verify>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1</automated>
    <fails_when>non-zero exit, or the summary line reads "0 passed" / "running 0 tests" (a zero count here means the feature gate excluded the module, not that the code is clean)</fails_when>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests" in the output — the new integration file compiling to zero tests means its `#![cfg]` header excluded it</fails_when>
    <automated>cargo test -p pmcp --all-features --lib classify_internal_method -- --test-threads=1</automated>
    <fails_when>non-zero exit, or fewer than 2 tests reported passed (the near-miss control and the raw-params assertion must both run)</fails_when>
    <automated>cargo build --all-features 2>&amp;1 | tail -5</automated>
    <fails_when>non-zero exit, or any line containing "error[" or "error:" in the output</fails_when>
    <automated>cargo build --no-default-features 2>&amp;1 | tail -5</automated>
    <fails_when>non-zero exit — proves the `#[cfg(not(...))]` arm of the `finalize_skills_resources` tuple change was updated</fails_when>
  </verify>

  <acceptance_criteria>
    - `cargo test --all-features --test skills_routing -- --test-threads=1` exits 0 with a nonzero passed count.
    - `grep -c 'dep:serde_yaml' Cargo.toml` returns at least 1, and `grep -n '^full' Cargo.toml` shows no `skills` token on the `full` or `full-v2` lines.
    - `grep -c 'skills = \["dep:serde_yaml"\]' Cargo.toml` returns 1.
    - `cargo test --all-features --test v1_severability_tripwire -- --test-threads=1` exits 0 — the enumerated feature lists were not disturbed.
    - The live-wire test asserts a digest matching `^sha256:[0-9a-f]{64}$` on the wire response body, not on an in-process struct.
    - `serde_yaml` is named in exactly one function body in `src/server/skills.rs`: `grep -c 'serde_yaml' src/server/skills.rs` returns 1 (D-04's single-swap-point requirement).
    - `cargo build --no-default-features` exits 0.
    - `src/server/skills.rs` contains no occurrence of the whole-value lowercase-hex format specifier applied to a digest value; the digest is produced by a per-byte width-2 fold.
    - `cargo test --doc --all-features skills -- --test-threads=1` exits 0 — the new `entries()` doctest runs and passes.
  </acceptance_criteria>

  <reversibility rating="costly">
    `Skills::entries()`, `SkillEntry` and `SkillResourceRef` become public 2.x API — additive now, but removal or a field-shape change later is a MAJOR break. Mitigated by `#[non_exhaustive]` on both structs and private fields plus accessors, which keeps later field additions MINOR.
  </reversibility>

  <done>
A live `StreamableHttpServer` carrying one frontmatter-bearing skill answers a
`skills/list` POST with a single conforming entry — verbatim frontmatter, a
`sha256:` + 64-lowercase-hex digest, and a byte-accurate size — on both a v2 and a
v1 framing, and the change is committed.
  </done>
</task>

<task type="auto">
  <name>Task 2: Routing guarantees — no public variant, no era gate, no routing name</name>

  <files>tests/skills_routing.rs</files>

  <read_first>
    - tests/skills_routing.rs — the file Task 1 created; extend it, do not replace it.
    - tests/v2_tasks_update_routing.rs:1196-1208 — `client_request_has_no_tasks_update_variant`, the source-scan tripwire idiom to copy verbatim in structure.
    - src/types/protocol/mod.rs — the `pub enum ClientRequest {` declaration and the rustdoc at ~:775-790 recording that it carries no `#[non_exhaustive]`, so `enum_variant_added` is a semver-MAJOR break.
    - src/types/mrtr.rs:343-352 — `name_bearing_key`, the single table both the `Mcp-Name` emitter and the server's cross-check resolve through.
    - src/server/streamable_http_server.rs:1312 — `is_name_bearing_method`, and its literal-contract test at ~:6112.
    - contracts/mcp-protocol-sdk-v1.yaml:746-792 — the v2 header cross-check contract, including the name-bearing method table at :751.
    - tests/common/v2.rs:736-760 — `v2_headers` and `v2_headers_for`, and `pmcp::testing::routing_name_key`, the production-table seam a test must resolve through rather than restating.
  </read_first>

  <action>
Extend `tests/skills_routing.rs` with the four routing guarantees the tracer path
does not itself prove. Each is a property the phase must keep true, not a behavior
it adds.

1. The semver source-scan tripwire. Copy the structure of
   `client_request_has_no_tasks_update_variant`: read
   `src/types/protocol/mod.rs` from the repo root, locate the
   `pub enum ClientRequest {` declaration, take the block up to the first
   column-0 brace terminator, and assert the block contains neither `SkillsList`
   nor `SkillsGet`. This source scan IS the enforcement — there is no
   `cargo semver-checks` in `Makefile` or `.github/workflows/` (grep returns zero
   hits), so nothing else catches the regression.

2. The runtime wire proof, spike 008's technique. Assert
   `serde_json::from_value::<ClientRequest>` on a frame with method `skills/list`
   is `Err`, the same on `skills/get` is `Err`, and — as the load-bearing control
   that proves the assertion measures routing rather than a malformed fixture —
   the same call on a `resources/list` frame is `Ok`. A wire proof without the
   passing control proves nothing.

3. The no-era-gate property. `server/discover` answers `-32601` on a v1
   connection; `skills/list` must not. Assert a v1-framed `skills/list` POST
   against the live harness returns a `result`, and add a negative control in the
   same test asserting a v1-framed `server/discover` POST against the same server
   DOES return error code -32601 — so the assertion cannot pass vacuously against
   a server that answers everything.

4. The not-name-bearing property, and why it is a tested non-change. Neither
   method appears in `src/types/mrtr.rs`'s name-bearing table, so the v2
   `Mcp-Name` header is discarded for them. `skills/get` carries a `uri` param,
   structurally identical to `resources/read`, which IS in the table — so the
   omission looks like an oversight and must be pinned as a decision. Assert
   `pmcp::testing::routing_name_key` yields nothing for both method strings, and
   assert a v2 `skills/list` POST built with `v2_headers_for` (which derives an
   empty `Mcp-Name` through the production table) is accepted at HTTP 200 rather
   than rejected with -32020. Record in the test's rustdoc that adding either
   method to the name-bearing table is a deliberate deferral, since it would
   require editing `contracts/mcp-protocol-sdk-v1.yaml`'s method table and the
   literal-contract test in `src/server/streamable_http_server.rs`.

Extend the module-doc property table at the top of the file with a numbered row per
new test, following the analog's discipline of stating which tests are not redundant
and what control run proved it.
  </action>

  <verify>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit, or the summary reports fewer than 6 tests passed (tracer's 2 plus this task's 4)</fails_when>
    <automated>cargo test --all-features --test v2_tasks_update_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit — the analog file must be unaffected by the shared-harness usage</fails_when>
  </verify>

  <acceptance_criteria>
    - A test named for the `ClientRequest` source scan exists in `tests/skills_routing.rs` and fails if either variant name is inserted into the `pub enum ClientRequest` block (verify by asserting the test reads `src/types/protocol/mod.rs` from disk, not from a compiled constant).
    - The wire-proof test asserts `Err` on both skills method strings AND `Ok` on `resources/list` in the same test body.
    - The no-era-gate test contains both the positive `skills/list` assertion and the negative `server/discover` -32601 control.
    - The name-bearing test resolves through `pmcp::testing::routing_name_key` rather than restating a method list.
    - `cargo test --all-features --test skills_routing -- --test-threads=1` reports at least 6 passed and 0 failed.
  </acceptance_criteria>

  <done>
`tests/skills_routing.rs` fails if a `skills/*` variant is ever added to the public
`ClientRequest` enum, if `skills/list` gains an era gate, or if either method
silently becomes name-bearing. Committed.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| HTTP client -> `classify_http_ingress` | Fully attacker-controlled JSON-RPC frame: method string, id, and params. |
| `skills/list` result -> caller | Discloses the full skill catalog: every skill name, description, author-written frontmatter field, and file URI. |
| Server author's SKILL.md text -> wire | Frontmatter is emitted verbatim, so anything the author wrote in it crosses the boundary. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-125-01 | Elevation of privilege | `classify_internal_method` / `classify_http_ingress` | high | mitigate | The classifier judges the METHOD only and never deserializes `params` — a malformed body becomes a `-32602` in the served branch AFTER the header/auth pipeline, never a parse error before it. Copied verbatim from the `TasksUpdate` discipline (`src/types/protocol/mod.rs:798-806`); asserted by the raw-params classifier test. |
| T-125-02 | Spoofing / Session fixation | `HttpIngress::is_initialize` | high | mitigate | The new variant joins the `false` alternation, so a `skills/list` POST can never mint a session (ASVS V3). Site (ii) of Task 1 step 7. |
| T-125-03 | Information disclosure | `skills/list` catalog projection | medium | mitigate | Entries are projected per request from the server's own registry at the same point the resource surface is reached; `Cacheable::Yes` is named at the projection call site only, never by adding a row to `request_is_cacheable`. Cross-authorization-context caching of a filtered listing is out of scope this phase because the shipped `Skills` registry is not authorization-filtered — recorded, not silently assumed. |
| T-125-04 | Information disclosure | Verbatim `frontmatter` emission | medium | accept | The draft REQUIRES verbatim emission (SEP §Frontmatter line 241), so redaction would be non-conformant. Mitigated by rustdoc on `Skills::entries()` warning that a secret placed in SKILL.md frontmatter is disclosed to every `skills/list` caller — previously it required a `resources/read`. |
| T-125-05 | Spoofing | `sha256:` digest presented as integrity | low | accept | The draft is explicit (line 267) that digests are unsigned and supplied by the same server that supplies the content, and that hosts MUST NOT treat a digest match as a security boundary. Rustdoc must say so; pmcp must not document its digests as an integrity guarantee. |
| T-125-SC | Tampering | `serde_yaml` 0.9 addition | medium | mitigate | Package-legitimacy verdict `OK` in 125-RESEARCH.md `## Package Legitimacy Audit`: crates.io since 2016-02-27, 6.8M weekly downloads, repo `github.com/dtolnay/serde-yaml`, already resolved in `Cargo.lock` at `0.9.34+deprecated` and already a production dep of four workspace crates — zero new packages enter the graph. `cargo audit` measured exit 0 with neither `serde_yaml` nor `unsafe-libyaml` among the 7 allowed warnings. No `[ASSUMED]` or `[SUS]` package is installed by this plan, so no blocking human checkpoint is required. |
</threat_model>

<verification>
- `cargo test --all-features -- --test-threads=1` (matches CI `.github/workflows/ci.yml:104`) exits 0.
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0 — `make lint` pins `--features "full"` and does not reach this module.
- `cargo test --doc --all-features -- --test-threads=1` exits 0 — `make test-doc` also pins `--features "full"`.
- Do NOT accept `make quality-gate` alone as this plan's verification: measured in 125-RESEARCH.md Pitfall 2, every one of its test legs pins `--features "full"`, which excludes `skills`, so a green gate here proves nothing about this code until 125-05 lands `make test-skills`.
</verification>

<success_criteria>
- A `skills/list` POST over streamable HTTP returns a conforming single entry with a verifiable digest.
- `ClientRequest` gains no variant; the source-scan tripwire and the runtime wire proof both pass.
- `skills/list` answers on v1 and v2 alike; `server/discover` still answers -32601 on v1.
- Neither method is name-bearing, and that is asserted rather than assumed.
- `cargo build --no-default-features` and `cargo build --all-features` both exit 0.
</success_criteria>

<output>
Create `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md` when done.
</output>

---

---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 02
type: execute
wave: 2
depends_on: ["125-01"]
files_modified:
  - src/types/protocol/mod.rs
  - src/server/streamable_http_server.rs
  - src/server/core.rs
  - src/server/mod.rs
  - src/server/builder.rs
  - tests/skills_routing.rs
autonomous: true
requirements: [D-06, D-07]
user_setup: []

estimate:
  tokens: 40000
  raw_tokens: 80000
  tasks: 2
  confidence: high

must_haves:
  truths:
    - "A live server answers a `skills/get` POST whose `params.uri` names a registered SKILL.md with a single entry identical in shape to a `skills/list` entry, carrying `resultType: \"complete\"` (D-07)."
    - "`skills/get` on a URI the server does not serve returns JSON-RPC error -32602 (Invalid params), per the current draft — NOT the -32601 the shipped `SkillsHandler::read` returns for `resources/read` (D-06)."
    - "`skills/get` with malformed or absent params returns -32602 from the served branch, after the header and auth pipeline has run, never as a classification-time parse error."
    - "The `skills/get` lookup is an exact-match against the entry map keyed by SKILL.md URI; the caller's URI is never joined, normalized, or manipulated into a path (ASVS V5)."
    - "A server built through `ServerCoreBuilder` answers both methods identically to one built through `Server::builder()` — neither build path returns -32601 while the other succeeds (RESEARCH Pitfall 6)."
    - "`request_is_cacheable` gains no row for either method; cacheability is named at each projection call site (D-07)."
  artifacts:
    - src/types/protocol/mod.rs
    - src/server/streamable_http_server.rs
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/builder.rs
    - tests/skills_routing.rs
  key_links:
    - "`classify_http_ingress` inner `match` -> `HttpIngress::SkillsGet` -> `Server::handle_skills_get` -> `build_skills_get_response`: the same five-site chain the tracer proved for `skills/list`, with the params now actually consumed."
    - "`finalize_skills_resources` -> `ServerCore.skill_entries` (this plan) and -> `Server.skill_entries` (125-01): the ONE function both build paths get entries from. If only one call site is updated, one builder's servers answer -32601 forever."
    - "`build_skills_get_response` error path -> `error_codes::INVALID_PARAMS`, deliberately diverging from the `METHOD_NOT_FOUND` its `build_discover_response` analog uses."
---

<objective>
Expand the tracer's proven path sideways: add `skills/get` on the same classifier
route, and close the twin-site gap so `ServerCoreBuilder`-built servers answer both
methods exactly as `Server::builder()`-built ones do.

Purpose: `skills/get` is the second of the two methods the draft makes MANDATORY
for any server declaring the extension. A server answering only `skills/list` is
still non-conformant. And a phase that wires only one of the two build paths ships
a server whose conformance depends on which builder its author happened to use.

Output: `skills/get` over HTTP with draft-correct `-32602` semantics, and both
build paths carrying entries from one function.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-CONTEXT.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-RESEARCH.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-PATTERNS.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md
</context>

## Artifacts this phase produces

Created in **this plan** (125-02). See 125-01-PLAN.md for the phase-wide table.

| Symbol / artifact | Kind | Location | Visibility |
|---|---|---|---|
| `InternalClientRequest::SkillsGet` | enum variant `{ params: serde_json::Value }` | `src/types/protocol/mod.rs` | `pub(crate)` |
| `HttpIngress::SkillsGet` | enum variant `{ id, params }` | `src/server/streamable_http_server.rs` | private |
| `assemble_skills_get_fast` / `assemble_skills_get_with_middleware` | HTTP response assemblers | `src/server/streamable_http_server.rs` | private |
| `build_skills_get_response` | shared projection free fn | `src/server/core.rs` | `pub(crate)` |
| `Server::handle_skills_get` | thin delegate | `src/server/mod.rs` | `pub(crate)` |
| `ServerCore::handle_skills_list` / `ServerCore::handle_skills_get` | thin delegates | `src/server/core.rs` | `pub(crate)` |
| `ServerCore.skill_entries` | struct field `Arc<IndexMap<String, SkillEntry>>` | `src/server/core.rs` | private |

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: skills/get over HTTP with draft-correct -32602 semantics</name>

  <files>src/types/protocol/mod.rs, src/server/streamable_http_server.rs, src/server/core.rs, src/server/mod.rs, tests/skills_routing.rs</files>

  <read_first>
    - .planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md — what the tracer actually landed, including the exact names of the entry map field and the projection fn.
    - src/types/protocol/mod.rs — the `SKILLS_GET_METHOD` constant and `InternalClientRequest::SkillsList` variant the tracer added, plus the `TasksUpdate` variant's raw-params rustdoc at ~:798-806.
    - src/server/streamable_http_server.rs — the five `HttpIngress::SkillsList` sites the tracer wrote; each needs a `SkillsGet` sibling. Also `assemble_tasks_update_fast` (:3888) and `assemble_tasks_update_with_middleware` (:3942), the params-consuming analog pair, and the `TasksUpdateCall` struct (:3831) showing the argument-bundling convention that avoids `clippy::too_many_arguments`.
    - src/server/core.rs — `build_skills_list_response` as the tracer wrote it, `build_discover_response` (~:2380) for the envelope shape, and the `request_is_cacheable` rustdoc (~:2153-2200) explaining why neither method gets a row.
    - src/server/skills.rs:556-575 — `SkillsHandler::read`, which returns `ErrorCode::METHOD_NOT_FOUND` for an unknown URI. That is the -32601 divergence D-06 records as an out-of-scope observation; do NOT copy it, and do NOT change it.
    - src/server/skills.rs:280-292 — `resolved_path` / `skill_md_uri`, and :321-357 `validate_reference_path`, the registration-time rules the lookup side must not re-open.
    - tests/skills_integration.rs:253 — `resources_read_unknown_uri_method_not_found`, the test that pins the pre-existing -32601 behavior this plan must leave passing.
    - tests/skills_routing.rs — the file to extend.
  </read_first>

  <behavior>
    - `classify_internal_method("skills/get", &garbage_params)` returns `Some(InternalClientRequest::SkillsGet { params })` with `params` byte-identical to the input, including when the input is a non-object or `Value::Null`. The classifier never rejects a body.
    - `classify_internal_method("skills/gets", ...)` returns `None`.
    - A `skills/get` POST whose `params.uri` equals a registered SKILL.md URI returns `result.skill` — a single object with the same `uri`, `frontmatter` and `resources` keys a `skills/list` entry carries — plus `result.resultType == "complete"`.
    - A `skills/get` POST whose `params.uri` names an unregistered URI returns error code -32602.
    - A `skills/get` POST whose `params.uri` names a registered skill's REFERENCE file (not its SKILL.md) returns -32602: the draft says the `uri` MUST be a skill's SKILL.md.
    - A `skills/get` POST with `params` absent, or with `params.uri` a non-string, returns -32602 and does not panic.
    - A `skills/get` POST whose `params.uri` contains `..` path segments or a trailing path suffix appended to a registered URI returns -32602 — the map lookup is exact-match, so no traversal reaches a file.
    - The `skills/get` result carries no pagination cursor key.
  </behavior>

  <action>
Add the second method on the route the tracer proved, consuming its params for the
first time in this phase.

1. `src/types/protocol/mod.rs`: add `InternalClientRequest::SkillsGet { params: serde_json::Value }`
   and its classifier arm keyed on `SKILLS_GET_METHOD`, cloning `params` and
   deserializing nothing. Copy the `TasksUpdate` variant's raw-params rustdoc
   reasoning: a classifier that deserialized would hand an unauthenticated caller a
   params error instead of an auth refusal, inverting the gate ordering. Extend the
   in-module classifier test with `skills/gets` as the near-miss control and a
   non-object params value proving pass-through.

2. `src/server/core.rs`: add `pub(crate) fn build_skills_get_response`, taking the
   entry map, the raw params, the id and the protocol context. Deserialize
   `params.uri` HERE, in the served branch. Four deltas from the
   `build_skills_list_response` sibling the tracer wrote.
   (a) On absent params, a non-object params, a missing `uri` key, or a non-string
   `uri`, return `ServerCore::error_response` with `error_codes::INVALID_PARAMS`
   and a message that does not echo the caller's raw URI unbounded (ASVS V7) —
   truncate or omit it.
   (b) Look the URI up by exact key in the entry map. Never join, normalize,
   percent-decode or otherwise manipulate the caller's string into a path (ASVS
   V5); registration-time `validate_reference_path` already closed the traversal
   surface and the lookup side must not re-open it.
   (c) On a miss, return `error_codes::INVALID_PARAMS` — that is -32602 per the
   draft (D-06). Do NOT use `error_codes::METHOD_NOT_FOUND`, which is what
   `build_discover_response` uses and what the shipped `SkillsHandler::read` returns
   for `resources/read`. Add a rustdoc paragraph recording that the `resources/read`
   -32601 divergence is a known, separately-tracked observation this function
   deliberately does not copy and does not fix.
   (d) On a hit, emit `{"skill": <entry>}` with `ResponseDisposition::Complete` so
   the result carries `resultType: "complete"`. Do NOT name `Cacheable::Yes`: the
   draft explicitly leaves the caching question open for `skills/get` (SEP line
   359). Name the non-cacheable claim at this call site and say in the rustdoc that
   the draft leaves it open, so a later phase can change it with a stated reason.
   Add no row to `request_is_cacheable` for either method — its `match` has no
   wildcard arm and its rustdoc calls such a row a lie about where the claim is
   made.

3. `src/server/mod.rs`: add `pub(crate) fn handle_skills_get` as a thin delegate
   over the shared projection, identical in shape to the `handle_skills_list` the
   tracer wrote. Zero logic; all gates live in `core.rs`.

4. `src/server/streamable_http_server.rs`: add `HttpIngress::SkillsGet { id, params }`
   at all five sites — variant declaration, the `is_initialize` `false` alternation,
   the `classify_http_ingress` fast-reject condition plus inner match arm, the v2
   header-gate alternation with no `method_override`, and the TWO per-path
   response-assembly arms via `assemble_skills_get_fast` and
   `assemble_skills_get_with_middleware`. If either assembler's argument list
   reaches eight, bundle the router inputs into a call struct as `TasksUpdateCall`
   does — that bundling exists because the count was measured against
   `clippy::too_many_arguments`, not anticipated.

5. `tests/skills_routing.rs`: add live-wire tests for every row of this task's
   `<behavior>` block. The unknown-URI, reference-URI, malformed-params and
   traversal-shaped-URI cases must each assert the numeric error code -32602
   explicitly, not merely that an error was returned. Add a control assertion in
   the same file that `resources/read` on an unknown URI still returns its
   pre-existing -32601 — the divergence is deliberate and must be visible as such
   rather than looking like an inconsistency someone should "fix".
  </action>

  <verify>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests" in the output, or fewer than 12 tests reported passed (6 from 125-01 plus this task's cases)</fails_when>
    <automated>cargo test --all-features --test skills_integration -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests" — in particular `resources_read_unknown_uri_method_not_found` must still pass unchanged</fails_when>
    <automated>cargo test -p pmcp --all-features --lib classify_internal_method -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" in the summary line</fails_when>
    <automated>cargo clippy --all-targets --all-features -- -D warnings 2>&amp;1 | tail -5</automated>
    <fails_when>non-zero exit, or any line containing "error:" or "warning:" attributable to src/server/streamable_http_server.rs or src/server/core.rs</fails_when>
  </verify>

  <acceptance_criteria>
    - A test asserts a `skills/get` response for an unregistered URI carries error code exactly -32602.
    - A test asserts a `skills/get` response for a registered skill's reference-file URI carries error code exactly -32602.
    - A test asserts `resources/read` on an unknown URI still carries error code exactly -32601, documenting the divergence as intentional.
    - `grep -c 'skills/list\|skills/get' src/server/core.rs` shows the method strings are referenced only through the `pub(crate)` constants or rustdoc, with no re-typed dispatch literal introduced in a `match` guard.
    - `src/server/core.rs`'s `request_is_cacheable` function body is unchanged: `git diff --stat src/server/core.rs` shows additions, and the `request_is_cacheable` match arms are byte-identical to the pre-plan version.
    - `HttpIngress::SkillsGet` appears at five distinct sites in `src/server/streamable_http_server.rs`: `grep -c 'HttpIngress::SkillsGet' src/server/streamable_http_server.rs` returns at least 5.
    - `cargo build --all-features` exits 0 — the deliberately-exhaustive inner `match` in `classify_http_ingress` compiled, proving no site was skipped.
  </acceptance_criteria>

  <done>
A live server answers `skills/get` with a conforming single entry, and returns
-32602 for an unknown URI, a reference URI, malformed params and a traversal-shaped
URI — while the pre-existing `resources/read` -32601 behavior is untouched and
pinned. Committed.
  </done>
</task>

<task type="auto">
  <name>Task 2: ServerCore twin-site parity — both build paths answer both methods</name>

  <files>src/server/core.rs, src/server/builder.rs, tests/skills_routing.rs</files>

  <read_first>
    - src/server/core.rs:452-512 — the `ServerCore` struct, including the `resources: Option<Arc<dyn ResourceHandler>>` field at ~:475 that the new entries field sits beside.
    - src/server/core.rs:2434-2440 — the twin-site parity rule: ONE shared unit called from BOTH native dispatch sites; `mod.rs` CALLS these helpers and never defines its own.
    - src/server/builder.rs:1426-1455 — `finalize_skills_resources` as 125-01 changed it to return a tuple, and its `ServerCoreBuilder::build` call site at :1356-1360 including the `#[cfg]` / `#[cfg(not)]` pair.
    - src/server/builder.rs:433-525 — `ServerCoreBuilder::skills` and `try_skills`.
    - src/server/mod.rs:5365-5380 — the paired `ServerBuilder::build` call site, so the two stay literally parallel.
    - 125-RESEARCH.md `### Pitfall 6: Two build paths, two places to thread the entries` — the cfg-asymmetry warning: `pub mod skills` is `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` while the `ServerBuilder` methods are plain `#[cfg(feature = "skills")]`. Preserve each site's existing gate; do not harmonize.
  </read_first>

  <action>
Close the gap that makes conformance depend on which builder the server author
chose.

1. `src/server/core.rs`: add a private `skill_entries: Arc<IndexMap<String, SkillEntry>>`
   field on `ServerCore`, placed beside the existing `resources` field and carrying
   a rustdoc line explaining that it is its own field rather than something derived
   from the `ResourceHandler`, because `ComposedResources` may wrap that handler and
   any downcast would silently report "no skills" the day a third composition layer
   appears.

2. `src/server/builder.rs`: populate the new field at the `ServerCoreBuilder::build`
   call site from the tuple `finalize_skills_resources` already returns, mirroring
   the `ServerBuilder::build` site line for line. Update the `#[cfg(not(...))]` arm
   to produce an empty map so the non-skills build keeps compiling.

3. `src/server/core.rs`: add `pub(crate) fn handle_skills_list` and
   `pub(crate) fn handle_skills_get` on `ServerCore` as thin delegates over the same
   `build_skills_list_response` / `build_skills_get_response` free fns the `Server`
   delegates call. Do not define a second projection — the parity rule is that the
   projection exists exactly once and both dispatch sites call it.

4. `tests/skills_routing.rs`: add a parity test that builds the SAME skill registry
   through both `Server::builder()` and `ServerCoreBuilder`, drives `skills/list`
   and `skills/get` against each, and asserts the two results are equal as
   `serde_json::Value` after normalizing the request id. A test that only checks
   both return success would pass while the two diverged in entry content; assert
   equality of the projected value.
  </action>

  <verify>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests", or the parity test is absent from the reported test names</fails_when>
    <automated>cargo test -p pmcp --all-features --lib server::builder -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" in the summary line</fails_when>
    <automated>cargo build --no-default-features 2>&amp;1 | tail -5</automated>
    <fails_when>non-zero exit — proves the `#[cfg(not(...))]` arm of the `ServerCoreBuilder` call site was updated alongside the tuple</fails_when>
    <automated>cargo build --all-features 2>&amp;1 | tail -5</automated>
    <fails_when>non-zero exit, or any line containing "error["</fails_when>
  </verify>

  <acceptance_criteria>
    - A test in `tests/skills_routing.rs` asserts value-level equality between the `skills/list` results produced by a `Server::builder()` server and a `ServerCoreBuilder` server carrying the same registry, and does the same for `skills/get`.
    - `grep -c 'skill_entries' src/server/core.rs` returns at least 3 (field declaration plus the two delegates).
    - `grep -c 'build_skills_list_response\|build_skills_get_response' src/server/core.rs` shows each projection fn is DEFINED once; `grep -c 'fn build_skills_list_response' src/server/core.rs` returns exactly 1 and `grep -c 'fn build_skills_get_response' src/server/core.rs` returns exactly 1.
    - `cargo build --no-default-features` exits 0.
    - `cargo test --all-features -- --test-threads=1` exits 0 with no newly failing test relative to the 125-01 baseline.
  </acceptance_criteria>

  <done>
A `ServerCoreBuilder`-built server and a `Server::builder()`-built server carrying
the same registry return byte-equal `skills/list` and `skills/get` projections, and
the equality is asserted rather than assumed. Committed.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| HTTP client -> `skills/get` `params.uri` | Fully attacker-controlled string used as a lookup key. This is the phase's only caller-supplied identifier. |
| HTTP client -> `skills/get` `params` body | Arbitrary JSON, including non-objects and absent params. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-125-06 | Tampering / Information disclosure | `build_skills_get_response` URI lookup | high | mitigate | Exact-match lookup in the entry `IndexMap` keyed by SKILL.md URI. No path join, no normalization, no percent-decode. Registration-time `validate_reference_path` already rejects `..`, leading `/`, `://` and null bytes; the lookup side does not re-open that. Asserted by the traversal-shaped-URI test returning -32602. |
| T-125-07 | Elevation of privilege | Gate-ordering inversion in the classifier | high | mitigate | `classify_internal_method` clones `params` and deserializes nothing, so a malformed `skills/get` body becomes a -32602 in the served branch AFTER the header/auth pipeline, never a parse error that would leak a params error to an unauthenticated caller. Asserted by the non-object-params classifier test. |
| T-125-08 | Information disclosure | Error message echoing the caller's URI | medium | mitigate | The -32602 message must not echo the raw caller-supplied URI unbounded (ASVS V7); truncate or omit it. |
| T-125-09 | Denial of service | Unbounded `skills/get` params | low | accept | Params are cloned once and read for a single string key; the frame size bound is the transport's existing body limit, unchanged by this plan. |
| T-125-10 | Spoofing / Session fixation | `HttpIngress::SkillsGet` minting a session | high | mitigate | The new variant joins the `is_initialize` `false` alternation, same as its `SkillsList` sibling (ASVS V3). |
</threat_model>

<verification>
- `cargo test --all-features -- --test-threads=1` exits 0 (matches CI).
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0.
- `cargo build --no-default-features` exits 0.
- `cargo test --all-features --test skills_integration -- --test-threads=1` exits 0 with `resources_read_unknown_uri_method_not_found` still passing — this plan changes no existing error code.
</verification>

<success_criteria>
- Both mandatory methods are answered over streamable HTTP.
- `skills/get` uses -32602 for every unresolvable-URI and malformed-params case.
- Both build paths carry entries from one function and return equal projections.
- No row is added to `request_is_cacheable`.
</success_criteria>

<output>
Create `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-02-SUMMARY.md` when done.
</output>

---

---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 03
type: execute
wave: 2
depends_on: ["125-01"]
files_modified:
  - src/server/skills.rs
  - tests/skills_integration.rs
autonomous: true
requirements: [D-02, D-05]
user_setup: []

estimate:
  tokens: 45000
  raw_tokens: 90000
  tasks: 3
  confidence: high

must_haves:
  truths:
    - "A skill's entry `resources` manifest lists the skill's own SKILL.md URI first followed by every registered reference URI — the manifest is complete, and every digest covers exactly the bytes `resources/read` returns for that URI (D-05)."
    - "Every emitted digest matches `^sha256:[0-9a-f]{64}$` and every emitted `size` equals the byte length of the served content, for arbitrary generated skills (property, not example)."
    - "Emitted `frontmatter` is verbatim: a nested `metadata:` object, a list-valued field and a non-required scalar field all survive the YAML-to-JSON round trip with their authored shapes, and the emitted object is never reconstructed from `resolved_description()` (which `with_description` can legitimately override)."
    - "LF-authored and CRLF-authored frontmatter blocks produce identical `frontmatter` JSON — the existing CRLF lock is preserved."
    - "A skill whose body carries no frontmatter block is EXCLUDED from `skills/list` entries, is still readable byte-identically via `resources/read`, and produces a build-time warning naming it — never a silently synthesized `{name, description}` (D-02)."
    - "`Skills` build-time validation REJECTS a skill whose final URI segment does not equal its frontmatter `name`, and only when a frontmatter `name` is present (ROADMAP SC#3, gap #4c)."
    - "A skill exceeding 512 resource entries or 16,777,216 total bytes produces a build-time warning; it is not rejected (ROADMAP SC#3, gap #5)."
    - "All 40+ existing frontmatter-less `Skill::new(...)` call sites — including the in-module proptest strategies and the duplicate-URI tests — keep compiling and keep passing (RESEARCH Pitfall 3)."
  artifacts:
    - src/server/skills.rs
    - tests/skills_integration.rs
  key_links:
    - "`Skills::entries()` -> `parse_frontmatter_value` -> `serde_yaml`: the single YAML seam. A second parse path anywhere would make the emitted frontmatter and the served SKILL.md capable of disagreeing, which the draft makes a hard host-side load failure."
    - "`SkillResourceRef.digest` / `.size` -> the exact bytes `SkillsHandler::read` returns for the same URI. Computing them from anything else makes the manifest and the served content disagree by construction."
    - "`Skills::entries()` and `Skills::into_handler()` must share one validation function, or a registry can produce entries it refuses to serve (or vice versa)."
---

<objective>
Make every entry the tracer proved on the wire actually complete and actually
validated: full `resources` manifests with per-file digests, verbatim frontmatter
including nested and list-valued fields, the D-02 warn-and-exclude path for
frontmatter-less skills, frontmatter name-identity rejection, and the SEP limits
warning.

Purpose: the tracer emitted a one-element manifest for a single skill. A conforming
host fetches every file in the manifest and compares the entry field-by-field
against what it reads; an incomplete manifest or a reconstructed frontmatter is a
guaranteed host-side rejection, not a graceful degradation.

Output: `Skills::entries()` produces entries a conforming host will accept, and
refuses to produce ones it would not.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-CONTEXT.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-RESEARCH.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-PATTERNS.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md
</context>

## Artifacts this phase produces

Created in **this plan** (125-03). See 125-01-PLAN.md for the phase-wide table.

| Symbol / artifact | Kind | Location | Visibility |
|---|---|---|---|
| `SkillDiagnostic` | enum: frontmatter-missing / limit-exceeded / name-mismatch | `src/server/skills.rs` | `pub(crate)` |
| `Skills::entries_with_diagnostics` | `pub(crate) fn (&self) -> Result<(Vec<SkillEntry>, Vec<SkillDiagnostic>)>` | `src/server/skills.rs` | `pub(crate)` |
| `Skills::validate_names` | `pub(crate) fn (&self) -> Result<()>` — the shared gap-#4c check | `src/server/skills.rs` | `pub(crate)` |
| `MAX_SKILL_RESOURCES` = 512, `MAX_SKILL_TOTAL_BYTES` = 16_777_216 | consts | `src/server/skills.rs` | private |

## Plan-time finding that supersedes RESEARCH Pitfall 3

**RESEARCH Pitfall 3 states that gap 4a — "final URI segment must equal
`Skill::name()`", checked unconditionally — "breaks nothing existing". That is
measurably false.** `grep -rn 'with_path(' src tests examples` at plan time found
six in-`src` call sites, of which **three would fail** under an unconditional 4a
reject:

- `src/server/skills.rs:846-853` — `Skill::new("a", "").with_path("p")` and
  `Skill::new("b", "").with_path("p")`, registered deliberately to prove
  duplicate-URI detection. Both violate 4a, so the test would receive a
  name-identity error instead of the duplicate-URI error it asserts.
- `src/server/skills.rs:864-871` — `Skill::new("b", "").with_path("a")` in the
  cross-skill reference-collision test. Final segment `a` != name `b`.
- `src/server/skills.rs:1144-1152` — `skills_strategy_with_refs`, which rewrites
  every generated skill's path to `p{i}` while keeping its generated name.
  Essentially **every** proptest case violates 4a.

Beyond the repo, `pmcp-book/src/ch12-8-skills.md:392` teaches
`.with_path("team/topic")` as an exercise on a differently-named skill, so 4a as a
reject is a breaking change to documented, taught usage.

**Resolution, and why it is not a scope reduction:** ROADMAP success criterion #3
scopes the reject precisely — "rejects a skill whose final URI segment does not
equal **its frontmatter `name`**". That is gap **4c**, which is conditional on
frontmatter being present, and none of the three breaking sites has frontmatter.
4c therefore ships as a hard reject, in full, exactly as the criterion states.
Gap 4a ships too — as a `tracing::warn!` with its own test — because no CONTEXT
decision authorizes turning it into a reject, SC#3 does not ask for it, and the
measurement above shows the reject would break unrelated existing behavior.
Promoting 4a to a reject belongs with the already-recorded "strict frontmatter
mode" deferral in `125-CONTEXT.md` `## Deferred Ideas`.

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Complete resources manifests and verbatim frontmatter</name>

  <files>src/server/skills.rs, tests/skills_integration.rs</files>

  <read_first>
    - .planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md — the exact shapes of `SkillEntry`, `SkillResourceRef`, `parse_frontmatter_value` and `sha256_digest_hex` as the tracer landed them.
    - src/server/skills.rs — `Skill` struct and accessors (:155-320), `reference_uri` (:288-300), `resolved_description` (:276) and `with_description` (:184-200) whose override semantics forbid reconstructing frontmatter from it, `Skills::into_handler` (:437-471) and its `IndexMap` ordering contract, `parse_frontmatter_description` (:656-680) for the BOM/CRLF handling to preserve, and `SkillsHandler::read` (:545-580) which defines the exact bytes each URI serves.
    - src/server/skills.rs:1116-1160 — the existing `skill_strategy` and `skills_strategy_with_refs` proptest strategies; the new digest property test should reuse them rather than minting a third generator.
    - tests/skills_integration.rs:41-70 — `build_widget_skill_lf` and `build_widget_skill_crlf`, the LF/CRLF fixture pair that locks the existing CRLF behavior (RESEARCH A4).
    - 125-RESEARCH.md `### Pattern 2: Entry synthesis at into_handler()` — the digest/size definition and assumption A6 (the digest covers the same bytes `resources/read` returns).
    - 125-RESEARCH.md `## Code Examples` — the SEP wire shape showing `resources` carrying the skill's own SKILL.md entry first, then supporting files.
  </read_first>

  <behavior>
    - A skill with two references produces an entry whose `resources` has exactly 3 elements: the SKILL.md URI first, then the two reference URIs in registration order.
    - Each `resources[i].digest` equals `"sha256:" + lowercase_hex(sha256(bytes_served_for_that_uri))` and each `resources[i].size` equals that byte slice's length.
    - A frontmatter block carrying `name`, `description`, a scalar `license`, a list-valued field and a nested `metadata:` mapping produces a `frontmatter` JSON object with all five keys, the list as a JSON array and the nested mapping as a JSON object.
    - A skill constructed with `Skill::new("x", body_whose_frontmatter_description_is_A).with_description("B")` emits `frontmatter.description == "A"` — the authored value, not the override.
    - The LF fixture and its CRLF twin produce `frontmatter` JSON that compares equal after only the skill-name difference is accounted for.
    - Entry order across a multi-skill registry equals registration order (the `IndexMap` contract), with no response-time sort.
  </behavior>

  <action>
Expand `Skills::entries()` from the tracer's one-element manifest to the complete,
conforming manifest.

1. `src/server/skills.rs`: for each skill, build `resources` as the skill's own
   `skill_md_uri()` first with digest and size over `skill.body().as_bytes()`,
   followed by one `SkillResourceRef` per registered reference in registration
   order, each keyed on `reference_uri(relative_path)` with digest and size over
   that reference's `body().as_bytes()`. These are exactly the byte slices
   `SkillsHandler::read` returns for the same URIs, so the manifest and the served
   content cannot disagree by construction — state that invariant in the rustdoc.
   Preserve `IndexMap` insertion order for entries; do not sort at response time,
   which the module header already documents as the deterministic-ordering
   contract.

2. Emit `frontmatter` from `parse_frontmatter_value` only. Never reconstruct it
   from `Skill::name()` or `Skill::resolved_description()`: `with_description` is
   an explicit override, so `resolved_description()` can legitimately differ from
   the SKILL.md's authored `description:` line, and the draft requires the emitted
   object to be identical to the file's. Add the reasoning as rustdoc on the
   emission site.

3. Add unit tests in the in-module test block covering every `<behavior>` row —
   name them so a `--lib skills` filter selects them. Add the multi-field verbatim
   test with a fixture carrying `name`, `description`, `license`, a list-valued
   field and a nested `metadata:` mapping. Add the LF/CRLF frontmatter equality
   test.

4. Add a proptest over the existing `skills_strategy_with_refs` asserting, for every
   generated registry that yields entries: every `digest` matches
   `^sha256:[0-9a-f]{64}$`, every `size` equals the length of the bytes the
   corresponding `resources/read` returns for that URI, and the manifest length
   equals `1 + references().count()`. Assert against the handler's actual read
   output rather than re-deriving the bytes in the test, so the property cannot pass
   by both sides making the same mistake.

5. `tests/skills_integration.rs`: add an integration-level assertion that the
   manifest for `build_widget_skill_lf` names all three URIs and that reading each
   one through the handler returns content whose length equals the manifest's
   `size` for it. Keep every existing test in this file passing unchanged.
  </action>

  <verify>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1</automated>
    <fails_when>non-zero exit, or the summary line reads "0 passed" / "running 0 tests"</fails_when>
    <automated>cargo test --all-features --test skills_integration -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests", or fewer than 11 tests passed (the file's existing 9 tests + 2 proptests must all still run)</fails_when>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit — the 125-01 wire assertions on digest shape and size must still hold with the expanded manifest</fails_when>
  </verify>

  <acceptance_criteria>
    - A `--lib skills` run reports a test asserting a 3-element manifest for a 2-reference skill, with the SKILL.md URI at index 0.
    - A proptest exists asserting the digest regex and the size-equals-served-bytes property, and it reads the served bytes back through the `ResourceHandler` rather than recomputing them from the `Skill`.
    - A test asserts `frontmatter.description` equals the authored frontmatter value on a skill that also called `with_description` with a different value.
    - A test asserts the LF and CRLF fixtures produce equal `frontmatter` JSON for every key other than the deliberately-differing name.
    - `grep -c 'resolved_description' src/server/skills.rs` shows no new occurrence inside the entry-synthesis function: `sed -n '/fn entries_with_diagnostics/,/^    }/p' src/server/skills.rs | grep -c resolved_description` returns 0.
    - `cargo test --all-features --test skills_integration -- --test-threads=1` exits 0 with every pre-existing test still present in the reported names.
  </acceptance_criteria>

  <done>
Every entry carries a complete `resources` manifest whose digests and sizes are
provably the served bytes, and verbatim frontmatter including nested and
list-valued fields. Committed.
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Warn and exclude frontmatter-less skills (D-02)</name>

  <files>src/server/skills.rs, tests/skills_integration.rs</files>

  <read_first>
    - src/server/skills.rs — the entry-synthesis function as Task 1 left it, plus the in-module test block's `Skill::new("x", "body")` constructions at :216, :244, :248, :306 and the unit tests using `Skill::new("a", "")`, `Skill::new("foo", "body")`, `Skill::new("zeta", "")`.
    - src/server/skills.rs:1116-1140 — `skill_strategy`, which generates `name` from `[a-z]{1,8}` and `body` from `[a-zA-Z]{0,20}`: arbitrary bodies that will essentially never contain valid frontmatter, and therefore the natural coverage for this path.
    - tests/skills_integration.rs:319-350 — the `Skill::new("propskill", body)` proptest, same situation.
    - src/server/core.rs:2217 and :1134 — the in-repo `tracing::warn!` idiom to follow.
    - 125-CONTEXT.md D-02 and 125-RESEARCH.md Pitfall 4 — why synthesizing `{name, description}` is a guaranteed host-side rejection rather than a graceful default (SEP §Integrity: the host MUST NOT load the skill on a field mismatch).
    - Makefile:1799-1802 — `check-todos` greps `src/` for TODO/FIXME/HACK/XXX. Deferrals go in rustdoc prose, never as a marker comment.
  </read_first>

  <behavior>
    - `Skills::entries()` on a registry of one frontmatter-bearing and one frontmatter-less skill returns exactly one entry, for the frontmatter-bearing skill.
    - The excluded skill's SKILL.md is still returned byte-identically by `resources/read` on its URI.
    - The excluded skill still appears in `resources/list`.
    - The exclusion produces a `SkillDiagnostic` naming the excluded skill's SKILL.md URI, and `Skills::entries()` emits a `tracing::warn!` carrying that URI.
    - A registry of only frontmatter-less skills yields an empty entry vector and no error — an empty listing is SEP-legal ("MAY return an empty or partial listing").
    - No entry is ever emitted whose `frontmatter` was synthesized from `Skill::name()` or `resolved_description()`.
  </behavior>

  <action>
Implement the D-02 warn-and-exclude semantics with an observable diagnostic, so the
behavior is unit-testable without installing a tracing subscriber.

1. `src/server/skills.rs`: add a `pub(crate) enum SkillDiagnostic` with a variant
   for a frontmatter-less skill carrying its SKILL.md URI. Add
   `pub(crate) fn entries_with_diagnostics(&self) -> Result<(Vec<SkillEntry>, Vec<SkillDiagnostic>)>`
   holding the real logic, and make the public `pub fn entries(&self)` a thin
   wrapper that calls it, emits one `tracing::warn!` per diagnostic naming the URI
   and the reason, and returns only the entries. Keeping the diagnostics
   crate-private adds no public API surface while making every warn path directly
   assertable from the in-module test block.

2. A skill whose `parse_frontmatter_value` returns `None` is skipped: no entry, one
   diagnostic. Do NOT hard-error — 40+ existing `Skill::new(...)` call sites, the
   in-module doctests, and both proptest strategies construct frontmatter-less
   skills, and D-02 chose partial listing over breaking them. Do NOT synthesize a
   `{name, description}` object: the draft makes a field-by-field mismatch a
   mandatory host-side refusal, so a synthesized entry ships a server that looks
   conformant and is unusable.

3. Record the strict-mode option in the `entries()` rustdoc as prose: a future
   fallible or strict variant may reject frontmatter-less skills once canonical
   surfaces are cleaned up. Write it as documentation, not as a TODO/FIXME/HACK/XXX
   marker — `make check-todos` greps `src/` for exactly those tokens and CLAUDE.md
   forbids self-admitted technical debt in code.

4. Add in-module unit tests for every `<behavior>` row, driving
   `entries_with_diagnostics` directly for the diagnostic assertions and
   `into_handler()` plus `read`/`list` for the still-served assertions.

5. `tests/skills_integration.rs`: add an integration test registering one
   frontmatter-bearing and one frontmatter-less skill, asserting `resources/list`
   still enumerates both SKILL.md URIs and `resources/read` on the excluded one
   returns its body byte-identically — the exclusion is from the skills listing
   only, never from the resource surface.
  </action>

  <verify>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" / "running 0 tests" in the summary line</fails_when>
    <automated>cargo test --all-features --test skills_integration -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests", or any pre-existing test name missing from the reported set</fails_when>
    <automated>make check-todos</automated>
    <fails_when>non-zero exit, or the output contains "Found technical debt comments"</fails_when>
  </verify>

  <acceptance_criteria>
    - A `--lib skills` test asserts a mixed registry yields exactly 1 entry and exactly 1 diagnostic.
    - A `--lib skills` test asserts a registry of only frontmatter-less skills yields 0 entries and `Ok`, not `Err`.
    - An integration test asserts the excluded skill is still in `resources/list` and still readable byte-identically.
    - `grep -c 'tracing::warn!' src/server/skills.rs` returns at least 1.
    - `make check-todos` exits 0 — no SATD marker was introduced for the strict-mode deferral.
    - Every pre-existing test in `src/server/skills.rs` and `tests/skills_integration.rs` still passes without edits to its assertions.
  </acceptance_criteria>

  <done>
A frontmatter-less skill is excluded from `skills/list` with a named warning, stays
fully readable through `resources/read`, and no entry anywhere carries a synthesized
frontmatter object. Committed.
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 3: Frontmatter name-identity reject and the SEP limits warning</name>

  <files>src/server/skills.rs</files>

  <read_first>
    - src/server/skills.rs:321-357 — `validate_reference_path`, the in-file validation-fn shape to copy: one `if` per rule, each returning `Error::validation` with the offending value interpolated.
    - src/server/skills.rs:437-471 — `Skills::into_handler`, which collects ALL duplicates before erroring rather than failing on the first. Match that aggregation style.
    - src/server/skills.rs:280-292 — `resolved_path` and `skill_md_uri`, which define what "final URI segment" means.
    - src/server/skills.rs:846-871 and :1144-1152 — the three call sites the "Plan-time finding" section above measured as breaking under an unconditional gap-4a reject. Read them before writing any name rule.
    - .planning/ROADMAP.md, the `### Phase 125:` success-criteria list, item 3 — the reject is scoped to the FRONTMATTER name.
    - 125-RESEARCH.md `## Code Examples` §Limits — 512 resource entries counted with SKILL.md included, and 16,777,216 bytes summed over the manifest's `size` values.
  </read_first>

  <behavior>
    - A skill whose body frontmatter carries `name: refunds` but whose resolved path's final segment is `billing` causes `Skills::entries()` and `Skills::into_handler()` to both return `Err(Error::Validation)` whose message names both the URI and the frontmatter name.
    - A skill with no frontmatter, or with frontmatter carrying no `name` key, is never rejected by the name rule regardless of its path.
    - Multiple mismatching skills in one registry produce a single error listing every offender, not just the first.
    - A skill whose resolved path's final segment differs from `Skill::name()` but whose frontmatter name matches (or is absent) is accepted, and produces a diagnostic and a `tracing::warn!` rather than an error.
    - A skill with 513 total resource entries produces a limit diagnostic naming its URI and the count; it is still emitted as an entry.
    - A skill whose manifest sizes sum above 16,777,216 produces a limit diagnostic naming its URI and the byte total; it is still emitted as an entry.
    - A skill with exactly 512 entries and exactly 16,777,216 bytes produces no limit diagnostic — the bounds are inclusive.
  </behavior>

  <action>
Add the two validation rules at the build-time choke point, with the reject scoped
exactly as ROADMAP success criterion 3 states.

1. `src/server/skills.rs`: add `pub(crate) fn validate_names(&self) -> Result<()>`
   implementing gap 4c only — for each skill whose `parse_frontmatter_value` yields
   an object with a string `name` key, compare that value against the final `/`
   segment of `resolved_path()`; on mismatch, record the offender. Aggregate all
   offenders and return one `Error::validation` naming every URI and its expected
   and actual name, matching `into_handler`'s existing collect-then-error style and
   `validate_reference_path`'s interpolation style. Call `validate_names` from BOTH
   `entries_with_diagnostics` and `into_handler`, so a registry can never produce
   entries it would refuse to serve.

2. Implement gap 4a as a DIAGNOSTIC, not a reject: when the final segment of
   `resolved_path()` differs from `Skill::name()`, add a name-mismatch
   `SkillDiagnostic` variant carrying both values, warned by `entries()` alongside
   the other diagnostics. The reject form is out of scope: ROADMAP criterion 3
   scopes the reject to the frontmatter name, three in-repo constructions
   (`src/server/skills.rs:846-853`, `:864-871`, `:1144-1152`) deliberately use a
   path whose final segment differs from the constructor name for reasons unrelated
   to naming, and `pmcp-book/src/ch12-8-skills.md:392` teaches that construction.
   State that scoping in the function's rustdoc as prose, and name the
   already-recorded strict-mode deferral as where promotion belongs — not as a
   TODO/FIXME/HACK/XXX marker.

3. Add private consts `MAX_SKILL_RESOURCES` = 512 and `MAX_SKILL_TOTAL_BYTES` =
   16_777_216 with a rustdoc citation to the SEP Limits section, and a
   limit-exceeded `SkillDiagnostic` variant. Count `resources` entries with the
   SKILL.md included and sum the manifest's `size` values. Exceeding either bound is
   a warning, never a rejection — ROADMAP criterion 3 says "warns". Both bounds are
   inclusive: 512 and 16,777,216 exactly are within limits.

4. Add in-module unit tests for every `<behavior>` row. For the limit tests,
   construct the oversized skill programmatically rather than embedding large
   literals, and keep the byte-total test under a size that would slow the suite —
   a handful of references whose bodies sum just past the bound is sufficient, since
   the guard sums declared sizes.
  </action>

  <verify>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" / "running 0 tests" in the summary line</fails_when>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1 2>&amp;1 | grep -c 'test result: ok'</automated>
    <fails_when>output is 0 — no "test result: ok" line means the filtered run produced no passing summary at all</fails_when>
    <automated>cargo test --all-features -- --test-threads=1</automated>
    <fails_when>non-zero exit, or any line beginning with "failures:" in the output — this is the full CI-matching run and must stay clean after the new rejects land</fails_when>
  </verify>

  <acceptance_criteria>
    - A test asserts a frontmatter `name: refunds` on a skill resolving to a path ending `billing` yields `Err(Error::Validation)` from BOTH `entries()` and `into_handler()`.
    - A test asserts a frontmatter-less skill with any path is accepted by the name rule.
    - A test asserts two mismatching skills produce one error message naming both.
    - A test asserts the 512-entry and 16,777,216-byte bounds are inclusive (no diagnostic at exactly the bound, a diagnostic one past it).
    - `src/server/skills.rs:846-853`, `:864-871` and the `skills_strategy_with_refs` proptest all still pass with their assertions unedited — confirm by running the full `--lib skills` filter and checking `test_1_8`, `test_1_8a` and `prop_1_17_no_reference_ever_listed` appear in the passing set.
    - `make check-todos` exits 0.
    - `cargo test --all-features -- --test-threads=1` exits 0.
  </acceptance_criteria>

  <reversibility rating="costly">
    Making frontmatter name-identity a hard `Err` changes `Skills::into_handler`'s
    accept/reject behavior for any downstream registry whose frontmatter name
    disagrees with its URI. It is required by ROADMAP success criterion 3 and is
    conditional on frontmatter being present, so no frontmatter-less construction is
    affected. Reversal is a one-function change; it is flagged, not gated.
  </reversibility>

  <done>
A frontmatter name that disagrees with its URI's final segment is rejected at build
time with an aggregated message; a constructor-name mismatch and an over-limit skill
each warn; every pre-existing skills test still passes unedited. Committed.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Server author's SKILL.md bytes -> `parse_frontmatter_value` | Arbitrary text reaching a YAML parser at build time. Not attacker-controlled in the usual sense, but arbitrary — a registry may be assembled from files on disk. |
| Registry size -> response size | The number and total byte size of skills determines the `skills/list` response size. |
| Emitted `frontmatter` -> every `skills/list` caller | Author-supplied fields cross to the wire verbatim. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-125-11 | Denial of service | `parse_frontmatter_value` on arbitrary bytes | medium | mitigate | The parse returns `Option` and never panics or unwraps on malformed input; a non-object or unparseable block yields `None`, taking the D-02 exclusion path. Asserted by the proptest over `skill_strategy`'s arbitrary bodies, and by the dedicated fuzz target 125-05 registers. |
| T-125-12 | Denial of service | Unbounded registry inflating `skills/list` | medium | mitigate | The SEP 512-entry / 16 MiB per-skill guard warns at build time so an operator sees the growth before a host does. Catalog-level pagination stays a recorded deferral (D-11) rather than an unstated gap. |
| T-125-13 | Information disclosure | Verbatim frontmatter carrying author secrets | medium | accept | Verbatim emission is required by the draft; mitigated by the rustdoc warning added in 125-01 and reinforced here on the nested-field path, where a `metadata:` block is the likeliest place a secret hides. |
| T-125-14 | Tampering | Manifest disagreeing with served bytes | high | mitigate | Digest and size are computed from the same `&str` bodies `SkillsHandler::read` returns for the same URIs, and the proptest reads the bytes back through the handler rather than recomputing them, so a divergence cannot pass by both sides making the same mistake. |
| T-125-15 | Spoofing | Digest read as an integrity guarantee | low | accept | SEP line 267: digests are unsigned and supplied by the same server that supplies the content; hosts MUST NOT treat a match as a security boundary. Documented in rustdoc; pmcp makes no integrity claim. |
</threat_model>

<verification>
- `cargo test -p pmcp --all-features --lib skills -- --test-threads=1` exits 0 with a nonzero passed count.
- `cargo test --all-features -- --test-threads=1` exits 0 (matches CI).
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0, including the cognitive-complexity budget: `entries_with_diagnostics` must stay under 25, which is why validation and diagnostics are separate functions rather than one nested loop.
- `make check-todos` exits 0.
</verification>

<success_criteria>
- Entries carry complete manifests whose digests and sizes are the served bytes.
- Frontmatter is verbatim, including nested and list-valued fields, on LF and CRLF.
- Frontmatter-less skills are excluded with a named warning and never synthesized.
- Frontmatter name-identity is rejected; constructor-name mismatch and SEP limit overruns warn.
- Every pre-existing skills test passes with unedited assertions.
</success_criteria>

<output>
Create `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-03-SUMMARY.md` when done.
</output>

---

---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 04
type: execute
wave: 3
depends_on: ["125-01", "125-02", "125-03"]
files_modified:
  - src/server/skills.rs
  - src/server/builder.rs
  - tests/skills_integration.rs
  - examples/s44_server_skills.rs
  - examples/c10_client_skills.rs
  - pmcp-book/src/ch12-8-skills.md
  - pmcp-course/src/part8-advanced/ch23-skills.md
  - pmcp-course/src/part8-advanced/ch23-exercises.md
  - pmcp-course/src/quizzes/ch23-skills.toml
autonomous: true
requirements: [D-03, D-08]
user_setup: []

estimate:
  tokens: 40000
  raw_tokens: 80000
  tasks: 3
  confidence: high

must_haves:
  truths:
    - "`resources/list` on a skills-backed handler returns exactly one entry per registered SKILL.md and nothing else — no synthesized discovery-index entry, and still no supporting-file entries (D-08, ROADMAP SC#2 and SC#4)."
    - "`resources/read` on the retired discovery-index URI returns the handler's ordinary unknown-URI error rather than a synthesized JSON document (D-08)."
    - "SKILL.md and every supporting file remain byte-identical through `resources/read`, and the dual-surface prompt-fallback byte-equality still holds on both LF and CRLF fixtures (ROADMAP SC#4)."
    - "`cargo run --example s44_server_skills` and `cargo run --example c10_client_skills` both exit 0, with c10 demonstrating `skills/list` in place of the retired index read (D-03, D-08, ROADMAP SC#4)."
    - "Every user-facing snippet — book chapter, course chapter, course exercises, course quiz — describes the method-based discovery surface, and every snippet skill body carries real frontmatter so it produces a conforming entry (D-03)."
    - "The `src/server/skills.rs` module doctest and the closing doctest of `pmcp-book/src/ch12-8-skills.md` remain byte-equal mirrors of each other after both are updated (the rule stated at `src/server/skills.rs:19`)."
  artifacts:
    - src/server/skills.rs
    - src/server/builder.rs
    - tests/skills_integration.rs
    - examples/s44_server_skills.rs
    - examples/c10_client_skills.rs
    - pmcp-book/src/ch12-8-skills.md
    - pmcp-course/src/part8-advanced/ch23-skills.md
    - pmcp-course/src/part8-advanced/ch23-exercises.md
    - pmcp-course/src/quizzes/ch23-skills.toml
  key_links:
    - "`SkillsHandler::new` list-push -> `resources/list` output -> 12 tracked assertion sites across 4 Rust files. Removing the push without updating all of them turns a correct change into a red suite."
    - "`src/server/skills.rs:19` -> `pmcp-book/src/ch12-8-skills.md` closing doctest: a byte-equality rule between a source file and a book chapter. They move together or the stated rule becomes false."
    - "`examples/c10_client_skills.rs:109-114` ASSERTS on the index read — unlike s44, which only prints. c10 panics rather than printing wrong output, so it is the load-bearing example edit."
---

<assumption_delta_decision>
**Primary noun:** skill discovery.
**Decision:** `promote`.
**Rationale:** Discovery moves from a singular synthesized index resource
(`skill://index.json`, agentskills.io discovery schema 0.2.0) to the method-based
`skills/list` the SEP-2640 working group chose. The method becomes the primary and
only discovery surface; the index resource is removed rather than kept alongside,
because it also violates the draft's URI structure rule (the index name is not a
skill name, and its `/SKILL.md` sibling does not exist). D-08 authorizes removal by
default, with a legacy gate only if a plan-time blast-radius check showed a consumer
needing it — the check was performed (12 tracked in-repo sites, all owned by this
repo, no external consumer known) and no legacy gate is required.
</assumption_delta_decision>

<objective>
Retire `skill://index.json` and bring every user-facing surface onto the method-based
discovery story, with real frontmatter everywhere a snippet registers a skill.

Purpose: with `skills/list` answering, the synthesized index is both redundant and
nonstandard. Leaving it served means a pmcp server advertises two discovery
surfaces, one of which the draft does not define — and one of which the draft's URI
rules forbid. The cleanup is not cosmetic: `examples/c10_client_skills.rs` ASSERTS on
the index read and will panic, and four documentation surfaces teach the retired
shape.

Output: one discovery surface, twelve updated assertion sites, two runnable examples,
and four documentation surfaces that describe what the code now does.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-CONTEXT.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-RESEARCH.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-PATTERNS.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-03-SUMMARY.md
</context>

## Artifacts this phase produces

This plan **removes** rather than creates. See 125-01-PLAN.md for the phase-wide
creation table.

| Symbol / artifact | Kind | Location | Action |
|---|---|---|---|
| `SKILL_INDEX_URI` | const | `src/server/skills.rs:58` | removed |
| `INDEX_JSON_MIME` | const | `src/server/skills.rs:60` | removed if it has no other consumer |
| `build_discovery_index_json` | private fn | `src/server/skills.rs:514-530` | removed |
| `SkillsHandler.index_json` | struct field | `src/server/skills.rs` | removed |
| the `list_resources.push(...)` index entry | statement | `src/server/skills.rs:499-503` | removed |
| the `if uri == SKILL_INDEX_URI` short-circuit | statement | `src/server/skills.rs:549` | removed |

## The measured blast radius — 12 tracked sites

Enumerated at plan time with `grep -rn 'skill://index.json\|SKILL_INDEX_URI\|build_discovery_index_json'`
over tracked source. **This list corrects 125-RESEARCH.md Pattern 3 in two
directions**, so use this table, not that one.

| File | Lines | Kind |
|---|---|---|
| `src/server/skills.rs` | 58, 60, 500, 504, 514, 549 | implementation |
| `src/server/skills.rs` | 804, 826, 913, 979, 987, 1111, 1240, 1392 | 7 unit-test assertions + 1 proptest URI list |
| `src/server/builder.rs` | 2302 | **1 unit-test assertion — ABSENT from RESEARCH Pattern 3** |
| `tests/skills_integration.rs` | 182, 228, 232, 366 | 3 tests, one of them a proptest |
| `examples/s44_server_skills.rs` | 19, 99 | doc header + `println!` |
| `examples/c10_client_skills.rs` | 109, 113 | a `read` call and **two `assert_eq!` that will PANIC** |
| `pmcp-book/src/ch12-8-skills.md` | 124, 181, 266, 384 | prose + snippet |
| `pmcp-course/src/part8-advanced/ch23-skills.md` | 171, 220 | prose |
| `pmcp-course/src/part8-advanced/ch23-exercises.md` | 35, 40, 101 | exercise instructions |
| `pmcp-course/src/quizzes/ch23-skills.toml` | 40 | **quiz answer text — ABSENT from RESEARCH Pattern 3** |

**Do NOT edit `pmcp-book/book/**` or `pmcp-course/book/**`.** They match the grep but
are untracked mdBook build output — `git ls-files` returns nothing for them. They
regenerate from `src/`.

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Retire the synthesized discovery index and update all 12 Rust assertion sites</name>

  <files>src/server/skills.rs, src/server/builder.rs, tests/skills_integration.rs</files>

  <read_first>
    - src/server/skills.rs:54-62 — the constant block holding the index URI and the two MIME constants; check whether `INDEX_JSON_MIME` has any consumer besides the index before removing it.
    - src/server/skills.rs:490-532 — `SkillsHandler::new` (the `list_resources.push` of the index entry and the `index_json` field assignment) and `build_discovery_index_json`.
    - src/server/skills.rs:534-580 — `SkillsHandler::list` and `read`, including the `if uri == SKILL_INDEX_URI` short-circuit at :549 and the unknown-URI error path at :556-575, which must remain the response for the retired URI.
    - src/server/skills.rs:800-830, 905-995, 1105-1115, 1235-1245, 1385-1395 — every unit-test assertion that names the index, including the length assertions (`resources.len()`), the positional assertions (`resources[2]`, `resources[3]`, `resources[1]`) and the proptest URI list at :1240.
    - src/server/builder.rs:2295-2310 — the `assert!(uris.contains(&"skill://index.json"))` assertion RESEARCH Pattern 3 omitted.
    - tests/skills_integration.rs:165-190 — `resources_list_returns_skill_md_and_index_only`, whose name and its `assert_eq!(result.resources.len(), 3, "2 SKILL.md + 1 index = 3")` both encode the retired shape.
    - tests/skills_integration.rs:220-240 — `resources_read_index_returns_resource_with_text_application_json`, which becomes meaningless and must be replaced rather than deleted silently.
    - tests/skills_integration.rs:345-380 — the proptest whose URI loop reads the index URI.
    - 125-CONTEXT.md D-08 — removal by default; a legacy gate only if a blast-radius check showed a consumer needing it. The check is in this plan's blast-radius table; no consumer needs it.
  </read_first>

  <behavior>
    - `resources/list` on a handler built from 2 skills returns exactly 2 resources, both SKILL.md URIs, in registration order.
    - `resources/list` still contains no supporting-file URI, for any registry (the existing "readable but not listable" property is unchanged).
    - `resources/read` on the retired discovery-index URI returns the handler's ordinary unknown-URI error, the same one any other unregistered URI gets.
    - `resources/read` on every registered SKILL.md and every supporting file returns byte-identical content to before this change.
    - The dual-surface byte-equality property still holds for both the LF and the CRLF fixtures.
  </behavior>

  <action>
Remove the synthesized discovery surface and re-point every assertion that measured
it.

1. `src/server/skills.rs`: delete the discovery-index URI constant, the
   `build_discovery_index_json` function, the `index_json` field on `SkillsHandler`,
   the `list_resources.push(...)` statement that appended the index entry in
   `SkillsHandler::new`, and the `read` short-circuit that served it. Delete
   `INDEX_JSON_MIME` too, unless a grep shows another consumer. After removal, a
   read of the retired URI must fall through to the existing unknown-URI error path
   unchanged — do NOT add a special case for it, and do NOT change that path's error
   code (the -32601 divergence is D-06's recorded out-of-scope observation).

2. `src/server/skills.rs` unit tests: update the eight assertion sites. Length
   assertions drop by one; positional assertions shift down by one index; the read
   test for the index becomes a test asserting the retired URI now yields the
   unknown-URI error; the proptest URI seed list at :1240 drops the index entry.
   Rename any test whose NAME encodes the retired shape so the name still describes
   what it asserts.

3. `src/server/builder.rs`: update the assertion at :2302 to assert the index URI is
   ABSENT from the built server's URI set, rather than present. Asserting absence
   rather than deleting the assertion keeps a regression detectable.

4. `tests/skills_integration.rs`: rename
   `resources_list_returns_skill_md_and_index_only` to describe SKILL.md-only
   listing, change its length assertion from 3 to 2 and update its message string,
   and add an explicit assertion that the retired index URI is absent from the list.
   Replace `resources_read_index_returns_resource_with_text_application_json` with a
   test asserting a read of the retired URI errors — a deleted test is a silent
   coverage loss, a replaced one is a pinned decision. Drop the index URI from the
   proptest's read loop at :366.

5. Do NOT introduce a legacy feature gate. The blast-radius check in this plan's
   table found 12 in-repo sites and no external consumer, which is the condition
   D-08 set for removal by default.
  </action>

  <verify>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" / "running 0 tests" in the summary line</fails_when>
    <automated>cargo test --all-features --test skills_integration -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests", or fewer than 11 tests passed</fails_when>
    <automated>cargo test -p pmcp --all-features --lib server::builder -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" in the summary line</fails_when>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit — the method-based discovery surface must be unaffected by removing the resource-based one</fails_when>
  </verify>

  <acceptance_criteria>
    - `grep -rc 'index.json' src/server/skills.rs src/server/builder.rs` shows occurrences only inside test assertions that assert ABSENCE or an error, and zero occurrences in the implementation region: `sed -n '1,600p' src/server/skills.rs | grep -c 'index.json'` returns 0.
    - `grep -c 'build_discovery_index_json' src/server/skills.rs` returns 0.
    - `grep -c 'index_json' src/server/skills.rs` returns 0.
    - A test asserts a `resources/list` on a 2-skill registry returns exactly 2 resources.
    - A test asserts reading the retired discovery-index URI returns an error rather than content.
    - The `src/server/builder.rs` assertion asserts absence, not deletion — `grep -c 'index.json' src/server/builder.rs` returns exactly 1.
    - `cargo test --all-features -- --test-threads=1` exits 0.
  </acceptance_criteria>

  <reversibility rating="costly">
    Removing a served resource is an observable behavior change for any downstream
    host that reads it. Authorized by D-08 and by ROADMAP success criterion 2, and
    the blast-radius check found no consumer outside this repo. Restoring it is a
    revert of one commit; flagged, not gated.
  </reversibility>

  <done>
The synthesized discovery index is gone from the implementation, all 12 tracked Rust
assertion sites measure the new shape (including two that assert its absence), and
every pre-existing byte-identity and not-listable property still passes. Committed.
  </done>
</task>

<task type="auto">
  <name>Task 2: Bring the two runnable examples onto the method-based discovery surface</name>

  <files>examples/s44_server_skills.rs, examples/c10_client_skills.rs</files>

  <read_first>
    - examples/s44_server_skills.rs — the whole file. Its doc header numbered list (:17-31) names the index as item 2, and its output block (:93-101) prints it. Note the three registered skills at :83-85.
    - examples/skills/hello-world/SKILL.md, examples/skills/refunds/SKILL.md, examples/skills/code-mode/SKILL.md — all three ALREADY carry `---`-delimited frontmatter with `name` and `description`, and all three names already match their resolved URIs' final segments (`refunds` under `.with_path("acme/billing/refunds")` resolves to final segment `refunds`). D-03's frontmatter requirement is therefore already satisfied for these three; verify this before assuming edits are needed.
    - examples/c10_client_skills.rs — the whole file, in particular the index read and its two `assert_eq!` at :107-114. This example ASSERTS, so it panics rather than printing wrong output.
    - .planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md and 125-02-SUMMARY.md — the exact entry shape and the `skills/list` / `skills/get` call surface available from an example.
    - Makefile:799-830 — `test-examples`, which BUILDS every example and is chained into `quality-gate` through `test-all`; and the run tests it feeds (`tests/docs04_examples_run.rs` and siblings) that execute example binaries.
  </read_first>

  <action>
Make both examples demonstrate the surface the code now serves.

1. `examples/s44_server_skills.rs`: remove the discovery-index line from the doc
   header's numbered list and renumber the remaining items, and remove the
   `println!` naming the auto-synthesized index. Replace both with the method-based
   story: state that the server answers `skills/list` and `skills/get`, and print
   the count of conforming entries the registry produced along with the first
   entry's URI and digest, so the example demonstrates the new surface rather than
   merely omitting the old one. Confirm the three embedded SKILL.md files already
   carry frontmatter with names matching their resolved URIs — per the read_first
   note they do, so no fixture edit should be needed; if a mismatch is found,
   correct the frontmatter, never the registration.

2. `examples/c10_client_skills.rs`: replace the index read and its two `assert_eq!`
   with the `skills/get` equivalent — fetch the entry for a registered SKILL.md URI
   and assert on the entry's `uri`, on its `frontmatter.name`, and on its
   `resources[0].digest` matching the `sha256:` + 64-lowercase-hex shape. Keep the
   example ASSERTING rather than only printing: an example that panics on
   regression is the strongest of the three doc surfaces, and that property is why
   this file is the load-bearing edit. Update the surrounding comment that describes
   `resources/list` as returning "SKILL.md + index ONLY".

3. Run both examples end to end and confirm the printed output describes the current
   behavior — the manual verification row in `125-VALIDATION.md` exists because the
   example asserts are partial and the printed narrative is not machine-checked.
  </action>

  <verify>
    <automated>cargo build -p pmcp --all-features --examples 2>&amp;1 | tail -5</automated>
    <fails_when>non-zero exit, or any line containing "error[" or "error:"</fails_when>
    <automated>cargo run --example s44_server_skills --features skills,full</automated>
    <fails_when>non-zero exit, or the output still contains the string "index.json", or the output contains "panicked"</fails_when>
    <automated>cargo run --example c10_client_skills --features skills,full</automated>
    <fails_when>non-zero exit, or the output contains "panicked" or "assertion", or the output still contains the string "index.json"</fails_when>
    <automated>make test-examples</automated>
    <fails_when>non-zero exit, or the output names an example that failed to compile</fails_when>
  </verify>

  <acceptance_criteria>
    - `grep -c 'index.json' examples/s44_server_skills.rs` returns 0.
    - `grep -c 'index.json' examples/c10_client_skills.rs` returns 0.
    - `cargo run --example c10_client_skills --features skills,full` exits 0 and its stdout contains a `sha256:` prefixed value.
    - `examples/c10_client_skills.rs` still contains at least three `assert_eq!` or `assert!` calls — the example remains self-checking.
    - `make test-examples` exits 0.
    - No file under `examples/skills/` was modified unless a frontmatter name genuinely mismatched its resolved URI; if one was, the SUMMARY names it and why.
  </acceptance_criteria>

  <done>
Both examples run clean, demonstrate `skills/list` and `skills/get`, name no retired
URI, and c10 still asserts on real wire shape. Committed.
  </done>
</task>

<task type="auto">
  <name>Task 3: Update the four documentation surfaces and keep the doctest mirror byte-equal</name>

  <files>pmcp-book/src/ch12-8-skills.md, pmcp-course/src/part8-advanced/ch23-skills.md, pmcp-course/src/part8-advanced/ch23-exercises.md, pmcp-course/src/quizzes/ch23-skills.toml, src/server/skills.rs</files>

  <read_first>
    - src/server/skills.rs:1-40 — the module header, including line 19's rule that the module doctest is a byte-equal mirror of the closing doctest in `pmcp-book/src/ch12-8-skills.md`, and the doctest body itself with its `assert!(prompt_text.starts_with("# Hello"))`.
    - pmcp-book/src/ch12-8-skills.md — lines 124 (the resources/list bullet naming the index), 181 (the "readable but not listable" paragraph), 266 (the index read snippet), 384 (the example-output paragraph), and the closing doctest at ~:360-375 whose `Skill::new("hello-world", "# Hello\nThis is a minimal skill.\n")` body carries NO frontmatter.
    - pmcp-course/src/part8-advanced/ch23-skills.md:165-230 — the two paragraphs describing the auto-synthesized index.
    - pmcp-course/src/part8-advanced/ch23-exercises.md:30-45 and :95-110 — three exercise instructions that tell the reader to assert on the index.
    - pmcp-course/src/quizzes/ch23-skills.toml:35-45 — the quiz answer text describing the expected `resources/list` contents.
    - Makefile:1348-1360 — the `book-test` target, which runs `mdbook test` over the book. Note it is NOT chained into `quality-gate`, so it must be run explicitly.
  </read_first>

  <action>
Bring all four tracked documentation surfaces onto the method-based discovery story,
and honour the mirror rule.

1. `pmcp-book/src/ch12-8-skills.md`: rewrite the four index-naming sites to describe
   `skills/list` and `skills/get` as the discovery surface, and `resources/list` as
   returning one entry per registered SKILL.md with supporting files readable but
   not listable. Replace the index-read snippet at ~:266 with a `skills/get`
   snippet whose shape matches what `examples/c10_client_skills.rs` now does, so
   the chapter and the example teach the same call.

2. Per D-03, give the chapter's closing doctest skill a real frontmatter block so
   the canonical book snippet produces a CONFORMING entry rather than one the D-02
   path excludes. The frontmatter `name` must equal the skill's resolved URI final
   segment or the 125-03 name-identity rule rejects it. The existing
   `assert!(prompt_text.starts_with("# Hello"))` will no longer hold once the body
   opens with a frontmatter delimiter — change the assertion to something the new
   body satisfies, and make the SAME change in the `src/server/skills.rs` module
   doctest so the byte-equal mirror stated at `src/server/skills.rs:19` remains
   true. Both sides move in this commit or the stated rule becomes false.

3. `pmcp-course/src/part8-advanced/ch23-skills.md`: rewrite the two index
   paragraphs the same way.

4. `pmcp-course/src/part8-advanced/ch23-exercises.md`: rewrite the three exercise
   instructions so the reader asserts on the method-based listing. Keep the
   exercises' difficulty and structure; change what they assert, not how many steps
   they have.

5. `pmcp-course/src/quizzes/ch23-quizzes` — specifically
   `pmcp-course/src/quizzes/ch23-skills.toml`: update the answer text so a reader
   who answers correctly against the current code is graded correct. A quiz whose
   right answer is the retired behavior is worse than no quiz.

6. Do NOT touch `pmcp-book/book/**` or `pmcp-course/book/**`. They match a grep for
   the retired URI but are untracked mdBook build output and regenerate from `src/`.
  </action>

  <verify>
    <automated>cargo test --doc --all-features -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" in the doctest summary — the updated module doctest must actually run</fails_when>
    <automated>make book-test</automated>
    <fails_when>non-zero exit, or the output contains "test result: FAILED"</fails_when>
    <automated>git ls-files -- pmcp-book/book pmcp-course/book</automated>
    <fails_when>the command prints any path — that would mean build output became tracked and this task's do-not-edit rule was violated in the wrong direction</fails_when>
    <automated>cargo test --all-features -- --test-threads=1</automated>
    <fails_when>non-zero exit, or any line beginning with "failures:"</fails_when>
  </verify>

  <acceptance_criteria>
    - `grep -rc 'index.json' pmcp-book/src/ pmcp-course/src/` returns 0 for every listed file.
    - `git status --porcelain pmcp-book/book pmcp-course/book` shows no staged or committed changes under those directories.
    - The `src/server/skills.rs` module doctest body and the closing doctest of `pmcp-book/src/ch12-8-skills.md` are byte-equal — verify by extracting both and diffing them; the diff must be empty.
    - The book chapter's closing doctest skill body opens with a frontmatter delimiter and its frontmatter `name` equals the skill's resolved URI final segment.
    - `cargo test --doc --all-features -- --test-threads=1` exits 0.
    - `make book-test` exits 0.
    - The course quiz's correct answer describes the method-based listing.
  </acceptance_criteria>

  <done>
Book chapter, course chapter, course exercises and course quiz all describe the
method-based discovery surface; the canonical book snippet carries conforming
frontmatter; and the source-to-book doctest mirror is byte-equal and passing.
Committed.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Retired resource URI -> `resources/read` | A caller may still request the removed URI. It must reach the ordinary unknown-URI path, not a special case. |
| Documentation -> server author | Snippets are copied into production servers. A snippet teaching a non-conforming construction propagates non-conformance. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-125-16 | Information disclosure | Removed index still reachable through a stale short-circuit | medium | mitigate | The `read` short-circuit is deleted, not merely emptied, so the retired URI falls through to the existing unknown-URI error path. Asserted by a replacement test and by the absence grep in the acceptance criteria. |
| T-125-17 | Tampering | Documentation teaching a non-conforming construction | medium | mitigate | D-03 gives every canonical snippet real frontmatter, so a reader who copies a snippet gets a skill that appears in `skills/list` rather than one silently excluded by the D-02 path. Enforced by `make book-test` and by the name-identity rule from 125-03 rejecting a mismatched snippet. |
| T-125-18 | Repudiation | Silent coverage loss when a test is deleted rather than replaced | low | mitigate | Both index-asserting tests are REPLACED with absence/error assertions rather than deleted, so a regression that reintroduces the index is still detectable. |
</threat_model>

<verification>
- `cargo test --all-features -- --test-threads=1` exits 0 (matches CI).
- `cargo test --doc --all-features -- --test-threads=1` exits 0.
- `make test-examples` exits 0 and both examples run to completion.
- `make book-test` exits 0.
- `grep -rc 'index.json' src/ tests/ examples/ pmcp-book/src/ pmcp-course/src/` shows occurrences only in the two absence/error assertions.
</verification>

<success_criteria>
- One discovery surface: the method-based one.
- Reading the retired URI errors through the ordinary path, and that is tested.
- All 12 tracked assertion sites updated; two of them assert absence.
- Both examples run clean and c10 still self-checks.
- Four documentation surfaces describe the shipped behavior, and the doctest mirror is byte-equal.
</success_criteria>

<output>
Create `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-04-SUMMARY.md` when done.
</output>

---

---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 05
type: execute
wave: 4
depends_on: ["125-01", "125-02", "125-03", "125-04"]
files_modified:
  - Makefile
  - fuzz/Cargo.toml
  - fuzz/fuzz_targets/fuzz_skill_entry.rs
  - tests/skills_routing.rs
  - src/server/skills.rs
autonomous: true
requirements: [D-01, D-09, D-10, D-11]
user_setup: []

estimate:
  tokens: 35000
  raw_tokens: 70000
  tasks: 3
  confidence: high

must_haves:
  truths:
    - "`make quality-gate` compiles AND runs the skills module's tests: a dedicated `make test-skills` leg is chained into the gate, and it FAILS rather than passing when it observes zero tests (D-09, RESEARCH Pitfall 2)."
    - "`skills` is added to neither the `full` nor the `full-v2` enumerated feature list, and `tests/v1_severability_tripwire.rs` still passes unchanged (D-09)."
    - "`make doc-check` reaches the skills module, so the phase's new rustdoc is warning-checked locally rather than only in CI."
    - "A fuzz target exercising entry synthesis on arbitrary bytes is registered in `fuzz/Cargo.toml`, its source file exists, and a source-scan test fails if either is removed — registration is verified without requiring a nightly toolchain (CLAUDE.md ALWAYS-fuzz requirement)."
    - "Arbitrary bytes as a SKILL.md body never panic entry synthesis, asserted on stable by a property test as well as by the fuzz target."
    - "Every deferral this phase makes is recorded in rustdoc prose and in the plan record, and NONE of them is a code TODO/FIXME/HACK/XXX marker — `make check-todos` exits 0 (D-01, ROADMAP SC#5, CLAUDE.md zero-SATD)."
    - "`set_skills_capabilities` keeps auto-declaring the extension with an empty object, and its rustdoc states both that the empty object means `directoryRead: false` and that the two mandatory methods are answered over streamable HTTP only (D-10, D-01)."
    - "The stdio-reach deferral names its owner and the measured hazard: over stdio the frame fails at `parse_message` and the server actor breaks its receive loop (D-01)."
  artifacts:
    - Makefile
    - fuzz/Cargo.toml
    - fuzz/fuzz_targets/fuzz_skill_entry.rs
    - tests/skills_routing.rs
    - src/server/skills.rs
  key_links:
    - "`Makefile` `quality-gate` -> `test-skills`: the gate is green on what it reaches, and until this leg exists the failures live in what it does not. Every gate leg that runs tests today pins `--features \"full\"`, which excludes `skills`."
    - "`test-skills` zero-test-count guard -> the project's recorded false-green class. A leg without the guard reports success on a run that executed nothing."
    - "`fuzz/fuzz_targets/fuzz_skill_entry.rs` -> `fuzz/Cargo.toml` `[[bin]]` -> the source-scan test: three artifacts that must agree, checked by the third."
---

<objective>
Close the loop: make the local quality gate actually see this module, satisfy the
CLAUDE.md ALWAYS-fuzz requirement for the new feature, and record every deferral
this phase makes as documentation rather than as debt.

Purpose: 125-RESEARCH.md Pitfall 2 measured that `make quality-gate` never compiles
or tests the skills module — every test leg pins `--features "full"`, which excludes
`skills`, so the gate reports green having run zero tests from this code. A phase
that ships four plans of new behavior behind a gate that cannot see it has not
shipped a gate at all. And ROADMAP success criterion 5 requires the two INFO-level
gaps to be explicitly deferred, never silently dropped.

Output: a gate leg that fails on zero tests, a registered fuzz target with a
nightly-free registration proof, and a complete deferral record in rustdoc.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-CONTEXT.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-RESEARCH.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-PATTERNS.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-04-SUMMARY.md
</context>

## Artifacts this phase produces

Created in **this plan** (125-05). See 125-01-PLAN.md for the phase-wide table.

| Symbol / artifact | Kind | Location | Visibility |
|---|---|---|---|
| `test-skills` | Makefile target, chained into `quality-gate` | `Makefile` | — |
| `fuzz_skill_entry` | fuzz target source | `fuzz/fuzz_targets/fuzz_skill_entry.rs` | fuzz crate bin |
| `fuzz_skill_entry` `[[bin]]` | registration stanza | `fuzz/Cargo.toml` | — |
| fuzz-registration source-scan test | `#[test]` | `tests/skills_routing.rs` | test crate |
| `skills` added to the `doc-check` feature list | Makefile edit | `Makefile:1320` | — |

## Plan-time finding: the contract-first check is already discharged

CLAUDE.md mandates updating the contract YAML in
`../provable-contracts/contracts/<crate>/` before implementing, and
125-RESEARCH.md assumption A3 flagged it as a Wave-0 item. Measured at plan time:

- `../provable-contracts/contracts/` exists but has **no `pmcp/` subdirectory**.
- The in-repo `contracts/` directory holds `binding.yaml`,
  `mcp-protocol-sdk-v1.yaml`, `team-servers-v1.yaml`, `pmcp-run/` and
  `team-servers/`. `grep -rln skills contracts/` returns **zero files**.
- `make comply` runs `pmat comply check --path .` as an INFORMATIONAL report
  (project-level advisories are non-blocking per CLAUDE.md D-07) and fail-closed
  enforces only `comply-bindings-check`, which is scoped to
  `contracts/team-servers/binding.yaml` and `crates/pmcp-team-servers/src`.

**No skills contract exists and none is required by any enforced gate.** Task 3
re-runs `make comply` as its own confirmation rather than assuming this holds.

**One adjacent contract note, recorded not actioned:**
`contracts/mcp-protocol-sdk-v1.yaml:751` enumerates the v2 name-bearing method
table (`tasks/get, tasks/update, tasks/cancel -> "taskId"`). 125-01 Task 2 asserts
that neither skills method is name-bearing. That is a deliberate non-change; adding
either method to the table would require editing this contract AND
`is_name_bearing_method`'s literal-contract test at
`src/server/streamable_http_server.rs:6112`, and belongs to a later phase.

<tasks>

<task type="auto">
  <name>Task 1: The `make test-skills` gate leg with a zero-test-count guard</name>

  <files>Makefile</files>

  <read_first>
    - Makefile:318-352 — `test-cargo-pmcp`, the repo's canonical "the gate was not reaching this code" leg. Its load-bearing part is the `ran=$$(... awk '/^test result:/ { total += $$4 } END { print total+0 }')` count extraction and the `if [ "$$ran" -eq 0 ]` guard with an explanatory failure message.
    - Makefile:1701-1727 — the `quality-gate` recipe, a flat `@$(MAKE) <leg>` list, and the `doc-check` leg's precedent comment: "Same shape as the test-cargo-pmcp leg -- the gate is green on what it reaches, and the failures live in what it does not."
    - Makefile:236, 777, 903 — `test-unit`, `test-doc` and `test-integration`, each pinning `--features "full"`. These are the legs that do NOT reach this module; do not modify them.
    - Makefile:1317-1321 — `doc-check`, whose explicit feature list omits `skills`. Note this list is NOT the `full`/`full-v2` enumerated pair, so extending it is safe.
    - Cargo.toml — the `full` and `full-v2` feature lines. Per D-09 neither may gain `skills`.
    - tests/v1_severability_tripwire.rs:1-30 — why `full` and `full-v2` are untouchable: the test derives both from `Cargo.toml` at test time and asserts their relationship.
    - 125-RESEARCH.md Pitfall 2 — the per-leg coverage table showing exactly which legs reach `skills` and which do not.
    - .planning/phases/.../125-VALIDATION.md — the sanctioned quick and full commands, and the explicit prohibition on `make test-unit` / `make test-integration` and on `cargo nextest -E 'test(...)'` as skills verifiers.
  </read_first>

  <action>
Add the gate's reach into this module, in the shape the repo already uses for
exactly this problem.

1. `Makefile`: add a `.PHONY: test-skills` target modelled line for line on
   `test-cargo-pmcp`. It must run the library tests, the doctests and the two
   skills integration binaries under a feature set that actually includes `skills`
   — use `--all-features`, or an explicit `--features` list naming `skills`
   alongside the transport features `tests/skills_routing.rs` requires. It must pass
   `-- --test-threads=1`, which CLAUDE.md mandates and which this workspace has
   recorded parallel-test races for. It must NOT use `make test-unit`,
   `make test-integration` or `cargo nextest -E 'test(...)'`: the first two pin
   `--features "full"` and report success having run zero tests from this module,
   and the nextest `test()` selector is a project-recorded false-green that silently
   matches zero tests and exits 0.

2. Carry the zero-test-count guard verbatim in shape from `test-cargo-pmcp`: sum the
   `test result:` lines' passed counts and fail with an explanatory message when the
   total is zero, naming the feature-gate cause. Because this leg runs more than one
   selector, also assert per-selector that each named test target actually reported
   a `test result:` line — the summed total staying nonzero from one selector is
   exactly how a second selector can go dark while the leg reports green, which is
   the failure `scripts/named-test-binary-count.awk` exists to catch for
   `test-cargo-pmcp`. Reuse that script if its output shape fits; otherwise assert
   each expected target name appears in the captured output.

3. Chain `test-skills` into the `quality-gate` recipe's flat leg list, placed after
   `test-all`, and carry a comment in the same framing as the `doc-check` leg's:
   the gate is green on what it reaches, and every one of its test legs pins
   `--features "full"`, which excludes this module.

4. Add `skills` to the `doc-check` target's explicit feature list at :1320 so the
   phase's new rustdoc is warning-checked locally under `RUSTDOCFLAGS="-D warnings"`.
   That list is not the `full`/`full-v2` enumerated pair and extending it disturbs
   no tripwire.

5. Per D-09 do NOT add `skills` to `full` or `full-v2`. Both are enumerated lists
   whose relationship `tests/v1_severability_tripwire.rs` derives from `Cargo.toml`
   and asserts; adding to one changes what the severance proof covers.

6. Add a `test-skills` line to the Makefile's help listing beside the other test
   targets.
  </action>

  <verify>
    <automated>make test-skills</automated>
    <fails_when>non-zero exit, or the output contains "reported 0 tests", or the final summary reports a total of 0 tests run</fails_when>
    <automated>make test-skills 2>&amp;1 | awk '/^test result:/ { t += $4 } END { print t+0 }'</automated>
    <fails_when>the printed number is 0 — the leg must actually execute tests, not merely exit 0</fails_when>
    <automated>cargo test --all-features --test v1_severability_tripwire -- --test-threads=1</automated>
    <fails_when>non-zero exit — the enumerated `full` / `full-v2` lists must be untouched</fails_when>
    <automated>grep -n '^full\|^full-v2' Cargo.toml</automated>
    <fails_when>either printed line contains the token "skills"</fails_when>
    <automated>make doc-check</automated>
    <fails_when>non-zero exit, or the output contains "warning:" — rustdoc warnings are zero-tolerance under RUSTDOCFLAGS="-D warnings"</fails_when>
    <automated>make quality-gate</automated>
    <fails_when>non-zero exit, or the output does not contain a "test-skills" leg banner — a gate that never invoked the new leg has not been chained</fails_when>
  </verify>

  <acceptance_criteria>
    - `grep -c '^test-skills:' Makefile` returns 1.
    - `grep -c 'test-skills' Makefile` returns at least 4 (target, phony, quality-gate chain, help listing).
    - The `test-skills` recipe contains a zero-count guard: `sed -n '/^test-skills:/,/^$/p' Makefile | grep -c 'ran.*-eq 0'` returns at least 1.
    - The `test-skills` recipe contains `--test-threads=1`.
    - The `test-skills` recipe contains neither `test-unit`, nor `test-integration`, nor a nextest `test(` selector.
    - `grep -n 'doc --no-deps' -A 2 Makefile` shows `skills` present in the doc-check feature list.
    - `grep -n '^full' Cargo.toml` and `grep -n '^full-v2' Cargo.toml` show no `skills` token on either line.
    - `make quality-gate` exits 0.
  </acceptance_criteria>

  <done>
`make quality-gate` now compiles, lints the rustdoc of, and RUNS the skills module's
tests, and fails loudly rather than green if it ever observes zero of them.
Committed.
  </done>
</task>

<task type="auto">
  <name>Task 2: Fuzz target for entry synthesis, with a nightly-free registration proof</name>

  <files>fuzz/fuzz_targets/fuzz_skill_entry.rs, fuzz/Cargo.toml, tests/skills_routing.rs, src/server/skills.rs</files>

  <precondition>`cargo fuzz run` requires a nightly toolchain and the `cargo-fuzz` binary, neither of which this repo's `rust-toolchain.toml` provides (`.github/workflows/fuzz.yml` removes that file and installs nightly explicitly). Assert before running any `cargo fuzz` command; if unavailable, the registration proof and the stable property test below are the acceptance evidence and no `cargo fuzz` invocation is required.</precondition>

  <read_first>
    - fuzz/fuzz_targets/fuzz_tasks_update.rs — the nearest phase-adjacent parser-shaped target; copy its structure, its `libfuzzer_sys::fuzz_target!` entry shape, and its module-doc convention of stating what invariant the target defends.
    - fuzz/Cargo.toml:1-70 — the standalone-workspace header, the `[dependencies.pmcp]` block with `default-features = false` and its explicitly narrowed feature list, and the rustdoc explaining why each feature is there. The `skills` feature must be added to that list or the target cannot reach the module.
    - fuzz/Cargo.toml:205-220 — the `[[bin]]` stanza shape (`name`, `path`, `test = false`, `doc = false`, `bench = false`).
    - .github/workflows/fuzz.yml:18-30 — the target matrix, which names four targets explicitly. A new target is NOT automatically fuzzed in CI; decide and record whether to add it to the matrix.
    - Makefile:786-797 — `test-fuzz`, which iterates `cargo fuzz list` but swallows every failure with `|| echo`. It cannot be this task's acceptance evidence.
    - tests/keyword_list_mirrors.rs:1-40 — the in-repo precedent for a `#[test]` that reads `fuzz/fuzz_targets/*` from disk as a drift gate. Copy that approach for the registration proof.
    - src/server/skills.rs — the `Skills::entries()` / `entries_with_diagnostics` surface as 125-03 left it, and the existing proptest block, so the stable property test lands beside its siblings.
  </read_first>

  <behavior>
    - `fuzz_skill_entry` takes arbitrary bytes, interprets them as a SKILL.md body, builds a one-skill registry, and calls the entry-synthesis path. It never panics, never unwraps a `Result`, and asserts the shape invariants: any emitted digest matches the `sha256:` + 64-lowercase-hex form, and any emitted `size` equals the corresponding body's byte length.
    - `cargo fuzz list`, when a nightly toolchain and `cargo-fuzz` are present, includes `fuzz_skill_entry`.
    - A `#[test]` in `tests/skills_routing.rs` fails if `fuzz/fuzz_targets/fuzz_skill_entry.rs` is missing, or if `fuzz/Cargo.toml` carries no `[[bin]]` stanza naming it, or if the two disagree on the path.
    - A stable-toolchain proptest asserts that a `Skill` built from an arbitrary UTF-8 body, including bodies with a leading BOM, a lone `---` line, an unterminated frontmatter block, and non-object YAML, never panics entry synthesis and always yields either an entry with a well-formed digest or a frontmatter-missing diagnostic.
  </behavior>

  <action>
Satisfy the CLAUDE.md ALWAYS-fuzz requirement with evidence that does not depend on
a toolchain the gate does not have.

1. `fuzz/fuzz_targets/fuzz_skill_entry.rs`: new target following
   `fuzz_tasks_update.rs`'s structure. Interpret the input bytes as a SKILL.md body
   (lossily, so non-UTF-8 input is exercised rather than rejected), build a
   single-skill registry, run entry synthesis, and assert the digest-shape and
   size-equality invariants on whatever it produces. Open with a module doc naming
   the invariant the target defends and why arbitrary bytes are the right input:
   skills content is untrusted input, and the YAML parse is the phase's only
   third-party parser reached from author-supplied bytes.

2. `fuzz/Cargo.toml`: add `skills` to the `[dependencies.pmcp]` feature list with a
   comment in the file's established style explaining that it gates
   `src/server/skills.rs` and without it the target has no seam to reach. Add the
   `[[bin]]` stanza with `test = false`, `doc = false`, `bench = false`, matching
   the sibling stanzas.

3. `tests/skills_routing.rs`: add the registration source-scan test, following
   `tests/keyword_list_mirrors.rs`'s approach of reading the fuzz tree from disk.
   Assert the target source file exists, that `fuzz/Cargo.toml` contains a `[[bin]]`
   stanza whose `name` is the target name, and that the stanza's `path` resolves to
   the file that exists. This is the acceptance evidence for registration —
   `make test-fuzz` cannot be, because it swallows every failure with `|| echo`, and
   `cargo fuzz list` needs a nightly toolchain the gate does not install.

4. `src/server/skills.rs`: add the stable-toolchain proptest described in
   `<behavior>`, placed in the existing proptest block. Include explicit
   regression-shaped cases alongside the generated ones — a leading BOM, a lone
   `---`, an unterminated frontmatter block, and frontmatter parsing to a YAML
   scalar or sequence rather than a mapping — since a generator will rarely produce
   these and they are precisely the malformed shapes a real SKILL.md hits.

5. Decide and RECORD whether `fuzz_skill_entry` joins `.github/workflows/fuzz.yml`'s
   four-target matrix. If it does not, say so in the SUMMARY with the reason, so its
   absence is a decision rather than an oversight.
  </action>

  <verify>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests", or the fuzz-registration test name is absent from the reported set</fails_when>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" / "running 0 tests" in the summary line</fails_when>
    <automated>test -f fuzz/fuzz_targets/fuzz_skill_entry.rs &amp;&amp; grep -c 'fuzz_skill_entry' fuzz/Cargo.toml</automated>
    <fails_when>non-zero exit, or the printed count is less than 2 (the `[[bin]]` name and its path both name the target)</fails_when>
    <automated>make test-skills</automated>
    <fails_when>non-zero exit, or the output contains "reported 0 tests"</fails_when>
  </verify>

  <acceptance_criteria>
    - `fuzz/fuzz_targets/fuzz_skill_entry.rs` exists and contains a `fuzz_target!` entry point.
    - `fuzz/Cargo.toml` contains a `[[bin]]` stanza naming `fuzz_skill_entry` with `test = false`, `doc = false`, `bench = false`, and `skills` appears in the `[dependencies.pmcp]` feature list.
    - A `#[test]` in `tests/skills_routing.rs` reads both `fuzz/Cargo.toml` and the target source from disk and fails if either is missing or they disagree on the path.
    - A proptest in `src/server/skills.rs` covers arbitrary bodies plus the four named malformed shapes and asserts no panic and a well-formed digest or a frontmatter-missing diagnostic.
    - `make test-skills` exits 0 with a nonzero test count.
    - The SUMMARY records whether the target was added to `.github/workflows/fuzz.yml`'s matrix, with a reason either way.
  </acceptance_criteria>

  <done>
A fuzz target for entry synthesis exists, is registered, is proven registered by a
test that needs no nightly toolchain, and its central invariant is additionally
asserted on stable by a property test covering the four malformed frontmatter
shapes. Committed.
  </done>
</task>

<task type="auto">
  <name>Task 3: Record every deferral in rustdoc — and nowhere else</name>

  <files>src/server/skills.rs</files>

  <read_first>
    - src/server/skills.rs:1-40 — the module header, where the transport-reach and deferral record belongs.
    - src/server/skills.rs:54-80 — `SKILLS_EXTENSION_KEY` and `set_skills_capabilities`, whose `json!({})` declaration already correctly means `directoryRead: false` and needs rustdoc, not a shape change.
    - src/shared/protocol_helpers.rs:32-42 — the seam rustdoc stating verbatim that `classify_http_ingress` is the ONLY production consumer, so an internally-routed method answers -32601 everywhere else including stdio, AND that the seam is transport-agnostic so a later plan can widen the reach without a semver break. Quote its substance in the deferral record.
    - src/server/mod.rs:1437-1478 — `run_transport_actor`, in particular the receive-error arm that BREAKS the loop, and the `request_tx` channel typed `(RequestId, Request)` over the PUBLIC enum. That channel type is why widening is a bigger change than the skills work itself.
    - src/shared/transport.rs:130-150 — `parse_method_message`, where the stdio frame's parse failure becomes a `TransportError::InvalidMessage`.
    - src/server/skills.rs:556-575 — `SkillsHandler::read`'s `METHOD_NOT_FOUND` for an unknown URI, the D-06 divergence to record as an observation.
    - 125-CONTEXT.md `<deferred>` — the five deferrals already agreed with the developer, each of which needs a home in rustdoc.
    - Makefile:1799-1802 — `check-todos` greps `src/` for TODO, FIXME, HACK and XXX. None of these words may appear in what this task writes.
    - contracts/mcp-protocol-sdk-v1.yaml:746-792 — the v2 header cross-check contract, for the name-bearing-table observation.
  </read_first>

  <action>
Write the phase's deferral record where a reader of the code will find it, in prose
that no debt scanner will flag.

1. `src/server/skills.rs` module header: add a section documenting transport reach
   per D-01. State that `skills/list` and `skills/get` are answered over streamable
   HTTP, that every other transport reaches requests through the public parse path
   which maps an internally-routed method to a method-not-found error, and record
   the MEASURED stdio behavior: the frame fails at the transport's message parse,
   becomes an invalid-message transport error, and the server actor's receive arm
   breaks the loop — so over stdio the connection tears down rather than answering.
   Name that widening the reach is a non-semver-breaking follow-on owned by the next
   skills phase of the v2.7 milestone, and name why it is bigger than the skills
   work itself: the actor's request channel is typed over the public request enum,
   so widening means changing that channel's type or adding a second one.

2. Same module header: record the remaining deferrals as a short prose list —
   `resources/directory/read` (legal to defer; the declaration already means the
   directory-read capability is off), the three client wrapper methods, strict
   frontmatter mode, catalog cursor pagination per D-11, the promotion of the
   constructor-name mismatch from a warning to a rejection per 125-03, and the
   observation that the resource-read path returns a method-not-found code where the
   draft's convention for an unknown resource is an invalid-params code. Each entry
   names WHAT is deferred and WHERE it is owned.

3. `set_skills_capabilities` rustdoc per D-10: state that the extension keeps being
   auto-declared, that the empty declaration object legitimately means the optional
   directory-read feature is not implemented, and that declaring the extension
   commits the server to both mandatory methods — which it now answers, over
   streamable HTTP. This is documentation only; the `json!({})` shape does not
   change.

4. Also record in the module header that neither skills method is name-bearing under
   the v2 routing-header cross-check, and that adding either would require editing
   both the protocol contract's method table and the transport's literal-contract
   test — so the omission is a decision with a stated cost, not an oversight.

5. Use NO occurrence of TODO, FIXME, HACK or XXX anywhere in what this task writes.
   `make check-todos` greps `src/` for exactly those four tokens and CLAUDE.md
   forbids self-admitted technical debt. Deferrals live in rustdoc and in the plan
   SUMMARY.

6. Re-run `make comply` and confirm the plan-time contract finding still holds — no
   `pmcp` skills contract exists and none of the enforced legs requires one. Record
   the confirmation in the SUMMARY.
  </action>

  <verify>
    <automated>make check-todos</automated>
    <fails_when>non-zero exit, or the output contains "Found technical debt comments"</fails_when>
    <automated>make doc-check</automated>
    <fails_when>non-zero exit, or the output contains "warning:"</fails_when>
    <automated>cargo test --doc --all-features -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" in the doctest summary</fails_when>
    <automated>make comply</automated>
    <fails_when>non-zero exit, or the output contains "BINDING DRIFT"</fails_when>
    <automated>make quality-gate</automated>
    <fails_when>non-zero exit</fails_when>
    <automated>cargo test --all-features -- --test-threads=1</automated>
    <fails_when>non-zero exit, or any line beginning with "failures:"</fails_when>
  </verify>

  <acceptance_criteria>
    - The `src/server/skills.rs` module header contains a transport-reach section naming streamable HTTP as the reach and describing the measured stdio loop-break behavior.
    - The module header lists at least six deferrals, each naming what is deferred and where it is owned.
    - `set_skills_capabilities`'s rustdoc states both the directory-read meaning of the empty declaration and the HTTP-only reach of the two mandatory methods, and its body still emits an empty JSON object: `sed -n '/fn set_skills_capabilities/,/^}/p' src/server/skills.rs | grep -c 'json!({})'` returns 1.
    - `grep -cE 'TODO|FIXME|HACK|XXX' src/server/skills.rs` returns 0.
    - `make check-todos` exits 0.
    - `make doc-check` exits 0 with the skills feature now in its list, proving the new rustdoc is warning-clean.
    - `make quality-gate` exits 0, and `cargo test --all-features -- --test-threads=1` exits 0.
  </acceptance_criteria>

  <done>
Every deferral this phase makes — stdio reach, directory read, client wrappers,
strict frontmatter, cursor pagination, the constructor-name promotion, the
resource-read error-code divergence, and the name-bearing-table non-change — is
recorded in rustdoc with an owner, and `make check-todos` exits 0. Committed.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Fuzz input bytes -> entry synthesis -> `serde_yaml` | Arbitrary, deliberately hostile bytes reaching a third-party YAML parser and a hashing path. |
| Quality gate output -> developer confidence | A gate leg that reports success on a run that executed nothing is a security control that lies. |
| Capability declaration -> host expectations | Declaring the extension commits the server to both mandatory methods; the declaration must not over-promise. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-125-19 | Denial of service | `serde_yaml` parse on hostile bytes | high | mitigate | `fuzz_skill_entry` drives arbitrary bytes through the entry-synthesis path with no unwrap, and a stable proptest covers the four named malformed frontmatter shapes. The parser is `serde_yaml` 0.9, whose transitive `unsafe-libyaml` is already in the graph today so this adds no new exposure, and `cargo audit` names neither. |
| T-125-20 | Repudiation | A gate leg that reports green having run zero tests | high | mitigate | The zero-test-count guard, plus a per-selector assertion that each named target reported a `test result:` line — the summed total staying nonzero from one selector is exactly how a second goes dark while the leg reports green. |
| T-125-21 | Spoofing | Capability declared but not honoured on a transport | medium | mitigate | D-10 keeps the declaration; the rustdoc states the reach explicitly so an operator can see that a stdio deployment does not get these methods. The alternative — declaring conditionally on transport — is not available: the capability is computed at build time, before a transport is chosen. |
| T-125-22 | Tampering | Deferral recorded as a code marker instead of documentation | low | mitigate | `make check-todos` fails the gate on any TODO/FIXME/HACK/XXX in `src/`, and the acceptance criteria assert a zero count in this file specifically. |
| T-125-SC | Tampering | No package-manager install in this plan | low | accept | This plan installs no crates.io package. The only dependency added by the phase is `serde_yaml`, audited and dispositioned as `T-125-SC` in 125-01-PLAN.md. No `[ASSUMED]` or `[SUS]` package is introduced, so no blocking legitimacy checkpoint applies. |
</threat_model>

<verification>
- `make quality-gate` exits 0 AND its output shows the `test-skills` leg running with a nonzero test count.
- `cargo test --all-features -- --test-threads=1` exits 0 (matches CI).
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0.
- `make doc-check` exits 0 with `skills` in its feature list.
- `make check-todos` exits 0.
- `make comply` exits 0 with no binding drift.
</verification>

<success_criteria>
- The local gate compiles, lints the rustdoc of, and runs this module's tests, and fails on zero tests.
- `full` and `full-v2` are untouched and the severability tripwire still passes.
- A fuzz target exists, is registered, and its registration is proven without nightly.
- Every deferral is in rustdoc with an owner, and no SATD marker exists.
</success_criteria>

<output>
Create `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-05-SUMMARY.md` when done.
</output>

## Review Instructions

**Verify against source — do not review the plan text in isolation.** The plans reference real files, migrations, routes, and tests in this repo.
1. Open the referenced files and check each claim against the actual code.
2. For every strength or concern, cite concrete `path/to/file:line` evidence plus the mechanism.
3. When a plan asserts a mechanism works (a guard, a query filter, a test that exercises a path), trace whether it actually does what is claimed — do not take the plan's word for it.
4. If you cannot read the repo (no file access), say so and downgrade that finding to an open question rather than asserting it.

Findings citing `file:line` evidence are weighted far more heavily than impressionistic ones; a review that only restates the plan's own claims has low value.

Analyze each plan and provide:

1. **Summary** — One-paragraph assessment
2. **Strengths** — What's well-designed (bullet points)
3. **Concerns** — Potential issues, gaps, risks (bullet points with severity: HIGH/MEDIUM/LOW)
4. **Suggestions** — Specific improvements (bullet points)
5. **Risk Assessment** — Overall risk level (LOW/MEDIUM/HIGH) with justification

Focus on:
- Missing edge cases or error handling
- Dependency ordering issues
- Scope creep or over-engineering
- Security considerations
- Performance implications
- Whether the plans actually achieve the phase goals

Output your review in markdown format.
