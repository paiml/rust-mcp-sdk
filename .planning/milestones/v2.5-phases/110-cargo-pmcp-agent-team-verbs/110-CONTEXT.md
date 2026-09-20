# Phase 110: cargo-pmcp Agent & Team Verbs - Context

**Gathered:** 2026-07-19
**Status:** Ready for planning

<domain>
## Phase Boundary

cargo-pmcp becomes the on-ramp for agents and teams, matching its existing server
story. This phase adds four developer verbs — `agent new`, `agent dev`, `team dev`,
and `package capture|show` — each a **thin CLI** over already-shipped crates
(`pmcp-agent` from Phase 108, `pmcp-team-servers` from Phase 109, `pmcp-package`
from Phase 107), with version-pin tripwire tests guarding against dependency drift.
Requirements CLI-01..04.

**In scope:** the four verbs, their scaffolding/runner output, the in-process
`team dev` demo, source/endpoint selection for `agent dev`, thin `package`
clients, and the pin tripwire tests. A cargo-pmcp version bump.

**Out of scope (belongs elsewhere):** book chapters / examples (Phase 111);
the platform-side capture API implementation, ECR registry, and import service
(platform-owned — this phase only ships thin *clients*); AgentCore deploy adapter
(deferred follow-on); any change to `pmcp-agent`/`pmcp-team-servers`/`pmcp-package`
public surfaces (this phase consumes them as-is — if a gap is found, note it, don't
widen scope here).

</domain>

<decisions>
## Implementation Decisions

### Command surface shape
- **D-01:** Ship the verbs as **dedicated nested subcommand groups** —
  `cargo pmcp agent new|dev`, `cargo pmcp team dev`, `cargo pmcp package capture|show`
  — mirroring the existing `workbook <cmd>` / `test <cmd>` / `add <cmd>` groups in
  `cargo-pmcp/src/main.rs`. This matches the goal's literal phrasing and keeps the
  growing verb set cleanly namespaced. Do NOT overload the top-level `new --kind`
  for agents.
- **D-01a (implementation reuse):** `agent new` MAY reuse the existing scaffolding
  engine that backs `new --kind sql-server`/`workbook-server` internally, but it is
  surfaced under the `agent` group, not as a `--kind`. One scaffolder, clean surface
  — the planner decides how much of `commands::new`/templates to share vs fork.

### `team dev` interaction model
- **D-02:** `team dev` is a **thin CLI over the Phase 109 `TeamRuntime` wiring API**
  (109 D-01) — it must NOT re-implement composition. Default behavior: run the
  doc-review E2E scenario (109 D-16) on a **FixedSource** and print a **labeled
  transcript** of the flow (member A drafts via team-fs → sync to review → ask
  approval → human resolves → member B reads + stores summary in mem-mcp, dispatched
  through team-mcp). Self-contained, deterministic, offline — the demo a developer
  runs first.
- **D-02a:** `--serve` is an **opt-in** flag that instead exposes team-mcp over HTTP
  (the HTTP-first binary path, 109 D-04's "expose externally" route) so a developer
  can drive the team from their own MCP client.
- **D-02b:** `--llm <endpoint>` (or reusing the `agent dev` source flag shape, D-03)
  swaps FixedSource for a real completion source; absent it, FixedSource is the
  default so `team dev` works with zero external services.

### `agent dev` source & defaults
- **D-03:** `agent dev` takes a `--source openai-compat|sampling|fixed` flag:
  - `openai-compat` (**default**) runs the agent loop against an OpenAI-compatible
    endpoint, defaulting to `http://localhost:11434/v1` (Ollama) with `--endpoint`
    override. Requires the `pmcp-agent` `openai-compat` feature.
  - `sampling` runs the agent as a **sampling-hosted server** (the Phase 108
    agent-as-server adapter over `ServerCore`, native-only) so an MCP host provides
    the LLM via `sampling/createMessage`.
  - `fixed` is a canned offline/CI mode (FixedSource) — no external LLM.
- **D-03a:** Default endpoint assumption is Ollama localhost (a common local dev
  setup), NOT auto-detection — explicit and predictable over magic. If the endpoint
  is unreachable, fail with an actionable message naming `--endpoint`/`--source`.

### `package capture|show` + platform dependency
- **D-04:** `package show` reads and renders a **local `.pmcp` package file fully
  offline** — no platform dependency (uses `pmcp-package`'s own parse/render). This
  is the always-works path.
- **D-04a:** `package capture` is a **thin client that requires a configured platform
  target** — reuse the existing `cargo pmcp configure` targets + `cargo pmcp auth`
  token cache rather than inventing new config. When no target/credentials are
  configured, fail with actionable guidance (name the `configure`/`auth` commands),
  never a silent stub or a hard panic.
- **D-04b:** `pmcp-package` is pinned at **`"0.1"` (caret)** per CLI-04; a pin
  tripwire test asserts the dependency version, matching the scaffold-pin tripwire
  pattern already in cargo-pmcp (0.17.3 era).

### Version-pin tripwires (pattern reuse, not a gray area)
- **D-05:** CLI-01 (`agent new`) ships a generated tripwire test pinning the
  scaffold's `pmcp-agent` dependency; CLI-04 pins `pmcp-package = "0.1"`. Both reuse
  the existing cargo-pmcp scaffold-pin tripwire mechanism — the planner mirrors it,
  does not invent a new one.

### Claude's Discretion
- Exact clap struct layout, flag naming beyond the pivotal ones above, help text,
  and how much of the existing `commands::new` scaffolder is shared vs forked.
- cargo-pmcp version bump magnitude (minor for new verbs) and the downstream dep
  version lines — resolve at plan/release time.
- Transcript formatting for the `team dev` demo (labels, coloring) — planner's call.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone / phase design
- `.planning/ROADMAP.md` §"Phase 110" — goal, CLI-01..04 success criteria, deps on 107/108/109.
- `docs/design/agents-teams-sdk-extraction-plan.md` §"cargo pmcp package capture|show" (line ~130) and the Package/platform ownership table (line ~23) — establishes that capture API / ECR / import are platform-owned; cargo-pmcp ships thin clients only.
- `.planning/REQUIREMENTS.md` CLI-01..04 — the traceable requirement text.

### Upstream crates this phase wraps (consume as-is; do NOT modify)
- `.planning/phases/108-pmcp-agent-loop-crate/108-CONTEXT.md` — locked agent-loop decisions: the two `agent dev` modes (openai-compat vs sampling-hosted), native-only hosted adapter (D-01/D-03/D-16), `CompletionSource`/`ToolInvoker`/`ConversationStore` seams.
- `crates/pmcp-agent/src/adapter/factory.rs` — `CompletionSourceFactory` / `SamplingSourceFactory` / `FixedSourceFactory` (the `--source` flag resolves to these).
- `.planning/phases/109-team-reference-servers/109-CONTEXT.md` — D-01 (`team dev` is a thin CLI over the wiring API), D-04 (in-memory transports; HTTP-first binaries expose externally), D-05/D-06 (composition-derived attachment), D-16 (doc-review E2E on FixedSource).
- `crates/pmcp-team-servers/src/compose/wiring.rs` — `TeamRuntimeBuilder`/`TeamRuntime` the `team dev` CLI drives; `crates/pmcp-team-servers/tests/small_team.rs` for the wiring pattern.
- `crates/pmcp-package/src/package/team.rs` — `TeamPackage { members, human_roles, built_in_servers, limits, entry_point, finalizer_agents }` (wire-frozen 0.1); and `pmcp-package` AgentPackage/parse for `package show`.

### cargo-pmcp patterns to mirror
- `cargo-pmcp/src/main.rs` (`enum Commands`, ~line 83) — the nested-subcommand-group pattern (`Workbook`/`Test`/`Add`) D-01 follows, and the `New`/`Dev` verb shapes.
- `cargo-pmcp/src/commands/new.rs` + `cargo-pmcp/src/templates/` — the scaffolding engine `agent new` reuses (D-01a).
- `cargo-pmcp/src/commands/configure/` + `cargo-pmcp/src/commands/auth_cmd/` — the target/auth config `package capture` reuses (D-04a).
- The existing scaffold-pin tripwire test (cargo-pmcp 0.17.3 era) — the mechanism D-05 mirrors.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cargo-pmcp/src/commands/new.rs` + `src/templates/`: the scaffolder behind `new --kind sql-server`/`workbook-server`; `agent new` reuses it internally (D-01a).
- `cargo-pmcp/src/commands/{configure,auth_cmd}`: named-target + token-cache config; `package capture` reuses these rather than new config (D-04a).
- `crates/pmcp-team-servers` `TeamRuntime` + the `small_team` test: the exact wiring `team dev` shells over (D-02).
- `crates/pmcp-agent` factories: `--source` maps to `OpenAiCompat`/`Sampling`/`Fixed` source factories (D-03).

### Established Patterns
- Nested clap subcommand groups (`Workbook`/`Test`/`Add`) — D-01 adds `Agent`/`Team`/`Package` the same way.
- Scaffold-pin tripwire test in generated projects — D-05 reuses it for `pmcp-agent`/`pmcp-package` pins.
- `dev` verbs build+run with live logs (`cargo pmcp dev --server`) — `agent dev`/`team dev` follow the same run-with-logs UX.

### Integration Points
- New `commands::agent` / `commands::team` / `commands::package` modules + three arms on `enum Commands` in `main.rs`.
- `agent dev` sampling mode connects to the native-only Phase 108 agent-as-server adapter; `openai-compat`/`fixed` do not.
- `team dev --serve` reuses the 109 HTTP-first server path; default (no `--serve`) stays fully in-process.

</code_context>

<specifics>
## Specific Ideas

- `team dev` default output is a *labeled transcript* of the 109 doc-review flow — the "it just works offline, watch the team collaborate" first-run experience.
- Ollama localhost (`http://localhost:11434/v1`) as the assumed default openai-compat endpoint — the common local-LLM dev setup.

</specifics>

<deferred>
## Deferred Ideas

- **AgentCore deploy adapter** — explicitly a deferred follow-on (roadmap); agents deploy via existing target adapters (an agent-as-server is just a server binary) in this milestone.
- **Platform-side capture API / ECR registry / import service** — platform-owned; this phase ships only thin clients. Any richer `capture` behavior beyond a thin client is platform scope.
- **Distributed / multi-process teams** — out of scope for the milestone ("small team, one process", 109); `team dev` is in-process.
- **`agent dev` auto-detect of the local endpoint** — considered and rejected in favor of explicit `--source`/`--endpoint` (D-03a); could revisit if users ask.
- **Interactive `team dev` REPL** — considered; deferred in favor of the deterministic scripted transcript + `--serve` for interactivity.

### Reviewed Todos (not folded)
None — no matching pending todos.

</deferred>

---

*Phase: 110-cargo-pmcp-agent-team-verbs*
*Context gathered: 2026-07-19*
