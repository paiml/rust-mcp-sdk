# Phase 109: Team Reference Servers - Context

**Gathered:** 2026-07-18
**Status:** Ready for planning

<domain>
## Phase Boundary

Ship the four team servers (team-fs, mem-mcp, approval-mcp, team-mcp) as open reference implementations with dev-grade backends in ONE feature-flagged crate `crates/pmcp-team-servers` (0.x, experimental, publishes after `pmcp-agent`), plus an in-process small-team wiring API, plus conformance tests proving each server's tool surface matches the Phase 107 PKG-03 contracts (`contracts/team-servers-v1.yaml`) — the same fixtures the platform servers can run. Requirements TEAM-01..06.

This phase ALSO aligns the SDK team model with the pmcp.run traces/team redesign (user-directed, 2026-07-18): a team is **N ≥ 1 AI agents + M ≥ 0 human members**, and collaboration-server attachment is **derived from composition, not configured per agent**. The SDK reference impl becomes the open, testable specification of that derivation rule.

Out of this phase: cargo-pmcp verbs (`team dev` CLI is Phase 110 — a thin CLI over this crate's wiring API), book chapters (Phase 111), any platform-side migration (traces/spans/capture/provenance from the redesign doc are platform scope), mem0-rust or any embedder dependency, distributed/multi-process teams (deploy-anywhere teams are "small team, one process").
</domain>

<decisions>
## Implementation Decisions

### Crate shape & dev-run surface
- **D-01:** The in-process small-team composition ships HERE as a **library wiring API** (all four servers + member agents in one tokio runtime, wired from a `TeamPackage`). Phase 110's `cargo pmcp team dev` becomes a thin CLI over it. The phase-goal "small team, one process" is proven by an integration test in this crate.
- **D-02:** The four dev binaries are **HTTP-first**: each binds an HTTP port by default (platform-endpoint shape, via pmcp streamable HTTP), with stdio available as a fallback flag.
- **D-03:** Binaries are configured from a **TeamPackage file** (`--package <file>`, pmcp-package `TeamPackage`) as the primary config — roster, members, per-server settings all come from the same definition Phase 110 and the platform use. Flags/env only override port + data dir. One definition format from laptop to platform.
- **D-04:** The in-process wiring API connects servers and members over **in-memory transports** (no sockets; deterministic tests; CI-sandbox safe). The HTTP-first binaries remain the way to expose any server externally.

### Composition-derived wiring (traces-redesign alignment)
- **D-05:** **Attachment is derived from team composition**, per the redesign §2.2: team-mcp attaches iff the team has **≥ 2 AI agents** (`members.len() >= 2`); approval-mcp attaches iff **≥ 1 human member** (`human_roles` non-empty; the channel initiator is implicit and never counted). `TeamPackage.built_in_servers` is demoted from source-of-truth to **opt-in extras**. Team-of-one with zero humans degenerates cleanly to "just the AgentServer" with zero wiring config.
- **D-06:** **team-fs and mem-mcp are both fully opt-in** (explicit `built_in_servers` entries). Only team-mcp and approval-mcp are derived. (The SDK takes the conservative reading of the redesign's §10 open decision — team-fs does NOT auto-ride with approval-mcp.)
- **D-07:** The derivation rule lives as a **pure, exported function in `pmcp-team-servers`** (e.g., `derive_attachment(&TeamPackage) -> AttachmentSet`, with a composition-snapshot type), property/unit-tested: N=1,M=0 → agent only; N≥2 → +team-mcp; M≥1 → +approval-mcp; opt-ins honored. Team-of-one is blessed by test. Snapshot-at-entry is the documented contract (composition resolves once at wiring/entry; membership edits affect the next run — trivially true for static dev config, but stated so the platform can adopt the same rule). `pmcp-package` stays a pure wire-format crate; policy evolves separately.

### Dev-backend fidelity
- **D-08:** `fs__get_download_url` on the local-directory dev backend returns a **`file://` URI** to the file's real path. The `TeamFsBackend` trait leaves URL semantics to each backend (platform keeps presigned S3).
- **D-09:** `fs__sync_to_review` / `fs__sync_from_review` get **real local semantics via a sibling `review/` directory**: workspace and review dirs side by side; sync_to_review copies out for a human to edit, sync_from_review copies edits back. A human can literally open the review dir in an editor.
- **D-10:** The **console (dev) approval channel** prints the ask (question, options, approval id) to the server's console; **resolution ALWAYS happens via the `resolve_approval` tool** from any connected client. One resolution path for both channels, no TTY dependency, deterministic in CI. No stdin prompting.
- **D-11:** The **webhook (CI) approval channel** is a **notify-only outgoing POST** (ask payload + approval id) to a configured URL; resolution still via `resolve_approval`. Channels are pure notification transports. Optional shared-secret header; no HMAC machinery (platform keeps DDB/EventBridge/HMAC).
- **D-12:** Approval ask/resolve records carry an **optional subject reference** (e.g., `subject_task_id`/`subject_ref`) stored on the approval record and echoed by `get_approval`/`resolve_approval` — so (agent output, human verdict) pairs stay linkable per redesign §3. The provisional contract YAML revs additively to include it.

### team-mcp member wiring
- **D-13:** team-mcp reaches members as **in-process Phase 108 `AgentServer` instances over in-memory MCP** — a `pmcp::Client` per member, full MCP hop. This exercises the real `ToolOutput::Result` + top-level `related_task` `_meta` path, which IS the TEAM-05 migration template replacing the platform's raw-JSON-RPC bypass.
- **D-14:** Depth + ancestor-chain guard state travels as **namespaced `_meta` fields on `tools/call`** (transport-agnostic); the HTTP binary maps the `x-pmcp-team-depth` header into that `_meta` at the edge. Guards work identically in-memory and over HTTP.
- **D-15:** Member agents get their LLM via **`CompletionSourceFactory` resolved from the member's `AgentPackage` llm `ConfigSlot`** through `SlotResolver` (Phase 108 D-14 machinery): OpenAI-compat (Ollama) for standalone dev, Anthropic if configured. No outer sampling host required to run a dev team.
- **D-16:** The phase-goal E2E scenario is a **doc-review flow** through all four servers: member A drafts a file via team-fs → syncs to review → asks approval via `team_approval__ask_*` → human resolves via `resolve_approval` → member B reads the file and stores a summary in mem-mcp — dispatched through `team_mcp__*` tools. Runs on FixedSource for CI determinism; real LLM optional.

### Conformance harness
- **D-17:** `pmcp-team-servers` ships an **exportable conformance harness** (module/feature): a fixture-driven runner (any server impl + fixture dir) used by this repo's tests AND importable by the platform as a dev-dependency against its operated servers. Fixtures stay canonical in `contracts/team-servers/fixtures/`.
- **D-18:** This phase **authors the binding.yaml and wires `pmat comply check`** for team-servers-v1 — closing the "DEFERRED to Phase 109" note in the contract metadata, per the house contract-first rule.
- **D-19:** The runner drives servers through a **real `pmcp::Client` over the in-memory transport**: initialize → `tools/list` (exact advertised set + schema equality) → `tools/call` per fixture. Proves "advertised == enforced" at the wire level; the platform can point the same runner at an HTTP endpoint.
- **D-20:** Fixture coverage target: **every tool + every guard** — at least one success dispatch fixture per advertised tool, an error fixture per contract error path (unknown member, malformed/excessive depth, self-call, ancestor-cycle, invalid args), and exact `tools/list` surface fixtures for all four servers.

### Claude's Discretion
- Module layout, feature-flag names, binary names, default ports, CLI flag spelling
- BM25/keyword scoring internals for the `TeamMemoryBackend` dev impl
- Exact namespaced `_meta` key names for depth/ancestry (follow existing pmcp `_meta` conventions)
- Composition-snapshot type shape and the `AttachmentSet` API
- Approval task lifecycle details on the in-memory `TaskStore` (reuse `with_task_store()` infrastructure)
- Fixture file layout/naming for the expanded coverage; how fixtures embed into the exportable harness (include_dir vs path)
- Contract YAML rev mechanics for the additive subject field + `_meta` depth documentation

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design & requirements
- `docs/design/agents-teams-sdk-extraction-plan.md` — approved milestone design; §4 Phase D (this phase: per-server plan, one-crate packaging), §5 publish order, §7 non-goals, §8.3 team-mcp bypass rationale
- `~/Development/mcp/sdk/pmcp-run/docs/DESIGN-agent-traces-consumers-lifecycle-provenance.md` — **the team redesign this phase aligns with (user-directed)**: §2 team model N≥1+M≥0 + composition-derived attachment, §3 human turns / subject linkage, §6.3 snapshot-at-entry semantics, §7 delta table, §10 open decisions. Traces/capture/billing sections are platform scope — read for context, do not implement.
- `.planning/REQUIREMENTS.md` — TEAM-01..06 definitions
- `.planning/phases/108-pmcp-agent-loop-crate/108-CONTEXT.md` — locked Phase 108 decisions the adapter composition builds on (D-10..D-16: task-augmented default, one-tool-per-member, fresh-run-per-call, SlotResolver, endpoint maps)

### Contracts & fixtures (PKG-03)
- `contracts/team-servers-v1.yaml` — the four tool-surface equations (11 `fs__*`, 6 `mem__*`, approval static+dynamic, `team_mcp__<member>` guards); metadata explicitly defers binding.yaml + pmat comply to this phase
- `contracts/team-servers/fixtures/` — Phase 107 conformance fixtures (representative set; this phase extends to every-tool + every-guard per D-20)
- `tests/team_contracts_conformance.rs` — Phase 107 structural conformance test (starting point for the wire-level harness)

### Code (verified 2026-07-18)
- `crates/pmcp-agent/src/adapter/server.rs` — `AgentServer`/`AgentServerBuilder` (the unit team-mcp composes N of)
- `crates/pmcp-agent/src/adapter/factory.rs` — `CompletionSourceFactory` / `SamplingSourceFactory` / `FixedSourceFactory` (D-15 rides these)
- `crates/pmcp-agent/src/config/` — `SlotResolver` + endpoint-map machinery (Phase 108 D-14/D-16)
- `crates/pmcp-package/src/package/team.rs` — `TeamPackage { members, human_roles, built_in_servers, limits (max_team_depth), entry_point, finalizer_agents }` — already the right wire shape for composition-derived attachment; wire-frozen 0.1, do NOT change serialization
- `src/types/tasks.rs` + `src/server/` task-store infrastructure — in-memory `TaskStore` for approval-mcp (`with_task_store()` precedent)
- `tests/common/duplex.rs` — in-memory/duplex transport convention (Phase 104-108) the wiring API and conformance runner build on

### Reference implementation (external, private repo — read, never copy)
- `~/Development/mcp/sdk/pmcp-run/built-in/agents-api/crates/mcp-team-server-core/` — the platform's operated team servers (tool surfaces, dispatch semantics, guard implementations). Boundary razor applies: study shapes, never copy code; S3/DDB/EventBridge/HMAC/mem0-rust stay platform-side.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AgentServer` (Phase 108) — team-mcp members are instances of this; no new agent machinery needed
- `CompletionSourceFactory` seams + `SlotResolver` — member LLM configuration is already-built Phase 108 machinery
- pmcp in-memory/duplex transport — the wiring API's member links and the conformance runner's drive path
- In-memory `TaskStore` + SEP-1686 task augmentation (`ToolOutput::Result`, `related_task` `_meta`, `poll_decision`) — approval-mcp tasks and member dispatch results
- pmcp streamable HTTP server (`http` feature) — the HTTP-first binaries
- `pmcp-tasks`/`pmcp-agent` isolation precedent — 0.x crate conventions, feature-gated optional deps

### Established Patterns
- `TaskStore` backend-trait pattern — contract in SDK, dev backend in SDK, operated backend platform-side; `TeamFsBackend`/`TeamMemoryBackend` follow it
- Feature-gated binaries + per-server feature flags (design §4 Phase D packaging decision)
- ALWAYS requirements: property + unit + fuzz + runnable example; `make quality-gate` before commits; PMAT cog ≤ 25
- Publish order: `pmcp-team-servers` publishes AFTER `pmcp-agent` (CLAUDE.md publish list gains a new entry)

### Integration Points
- `crates/pmcp-team-servers/` — new workspace member (regular member; note only root `pmcp` is clippy-gated today)
- `contracts/` — binding.yaml + expanded fixtures land beside team-servers-v1.yaml
- Phase 110 `cargo pmcp team dev` — consumes the wiring API; keep it CLI-thin
- Platform migration path — the exportable conformance harness + pure derivation fn are the two artifacts pmcp.run imports

</code_context>

<specifics>
## Specific Ideas

- **The SDK as the open spec of the platform's derivation rule** (user-directed via the traces redesign doc): pmcp.run is moving to composition-derived server attachment; the reference wiring API implements the same rule as a pure tested function, so the platform converges on tested-in-the-open behavior. `help-team` (one member, role `entry_point`) is live proof team-of-one already works.
- "One definition format from laptop to platform" — binaries, wiring API, Phase 110 CLI, and the platform all configure from the same `TeamPackage`.
- The doc-review E2E flow is the narrative demo: it exercises every contract surface (fs write/sync, approval ask/resolve, mem add, team dispatch with `related_task` `_meta`) in one story a BA can follow.

</specifics>

<deferred>
## Deferred Ideas

- **Sampling passthrough up the chain** (member sampling proxied through team-mcp to the outermost client's LLM) — elegant key-less model hosting; requires sampling-proxy machinery; revisit after this phase
- **HTTP download route with expiring tokens** for the team-fs dev backend (behaviorally closer to presigned URLs than `file://`) — if the dev loop ever needs remote-fetchable URLs
- **Inbound webhook callback endpoint** for approval resolution — production-shaped CI approvals; notify-only POST suffices for dev-grade
- **Nested-team demo** (a team as a member of another team) — the depth/ancestor guards are implemented per contract, but no nested example this phase
- **team-fs auto-attach with approval-mcp** — the redesign's leaning (§10); SDK chose fully opt-in; revisit if the platform lands the auto-attach default
- **Traces-redesign platform items** — human-turn span kinds, capture policy defaults, provenance stamps, billing hardening: platform scope, not SDK
- Per-target deploy demos of team servers — Phase 110/111 territory

</deferred>

---

*Phase: 109-team-reference-servers*
*Context gathered: 2026-07-18*
