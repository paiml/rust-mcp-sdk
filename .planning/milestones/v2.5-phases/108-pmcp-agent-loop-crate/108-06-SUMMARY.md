---
phase: 108-pmcp-agent-loop-crate
plan: 06
subsystem: api
tags: [agent-as-server, task-lifecycle, sampling-source, completion-source-factory, wasm-gate, package-composition, shape-mapping]

# Dependency graph
requires:
  - phase: 108-pmcp-agent-loop-crate (plan 03)
    provides: "AgentEngine + RunOutcome/IterationResult + TurnMessage + decide::* pure functions"
  - phase: 108-pmcp-agent-loop-crate (plan 04)
    provides: "SamplingSource (native-only over PeerHandle) + OpenAiCompatSource/AnthropicSource + SecretString"
  - phase: 108-pmcp-agent-loop-crate (plan 05)
    provides: "ClientToolInvoker + ConnectorClient seam + resolve_agent + SlotResolver/ProgrammaticBuilder"
  - phase: 108-pmcp-agent-loop-crate (plan 01)
    provides: "pmcp 2.17.0 on_sampling_with_tools + sample_with_tools + store-backed task lifecycle on the high-level Server"
provides:
  - "AgentServer — one package-driven, task-supported tool on the high-level pmcp::Server with a REAL store-backed task lifecycle (create -> completed), a derived description, per-request completion source, run_id resume, stateless per call (AGNT-07, D-10/11/12)"
  - "CompletionSourceFactory seam + SamplingSourceFactory (request-scoped SamplingSource from extra.peer()) + FixedSourceFactory (preconstructed HTTP/mock source)"
  - "Arc<dyn Seam> blanket impls (CompletionSource/ToolInvoker/ConversationStore) so the generic engine runs over an erased per-request source + shared invoker/store"
  - "examples/s50_standalone_vs_sampled — one engine, two sources, network-free under default features (Anthropic at compile level; opt-in live Ollama)"
  - "tests/real_loop_sampling (D-03 full loop over SamplingSource) + tests/e2e_package_to_adapter (AGNT-09 package->adapter round-trip)"
  - "ci.yml pmcp-agent-targets gate: wasm32 default-feature build + native openai-compat,anthropic,url-connector build, wired into the required `gate`"
  - "docs/design/pmcp-agent-vs-durable-lambda-mapping.md — D-09 shape-compatibility mapping (no private code copied)"
affects: [phase-109-team-servers, AGNT-04, AGNT-05, AGNT-06, AGNT-07, AGNT-09]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Agent-as-server on the high-level pmcp::Server: .task_store() gives the REAL store-backed task lifecycle AND Server::run wires the request-scoped peer, so one served instance supports both the task lifecycle and hosted sampling (a single ServerCore-over-pump instance would have no peer)"
    - "Synchronous task completion: a with_task_support(Required) tool returns a task-shaped value {taskId,status,result}; the SDK create-path (extract_terminal_result) mints its own store id, persists the nested CallToolResult, and completes the task"
    - "Request-scoped source via a factory: the tool handler asks CompletionSourceFactory for THIS request's source (SamplingSource from extra.peer(), or a fixed HTTP source) instead of holding a preconstructed source"
    - "Arc<dyn Seam> blanket impls let a generic AgentEngine<C,T,S> run over an erased per-request completion source plus shared invoker/store; the ToolInvoker impl forwards invoke_batch so a concrete override is preserved"
    - "Continuity via the store (D-12): run_id (or conversation_id) resumes prior history; absent, a collision-safe uuid is minted at the adapter boundary (an EFFECT, never inside decide.rs)"
    - "Description derivation rule (AgentPackage has no description field): first sentence of instructions, else 'Run the {name} agent.'"

key-files:
  created:
    - crates/pmcp-agent/src/adapter/factory.rs
    - crates/pmcp-agent/src/adapter/server.rs
    - crates/pmcp-agent/tests/adapter_agent_as_server.rs
    - crates/pmcp-agent/tests/real_loop_sampling.rs
    - crates/pmcp-agent/tests/e2e_package_to_adapter.rs
    - crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs
    - docs/design/pmcp-agent-vs-durable-lambda-mapping.md
  modified:
    - crates/pmcp-agent/src/adapter/mod.rs
    - crates/pmcp-agent/src/lib.rs
    - crates/pmcp-agent/src/seams/completion.rs
    - crates/pmcp-agent/src/seams/tool.rs
    - crates/pmcp-agent/src/seams/store.rs
    - crates/pmcp-agent/src/sources/mod.rs
    - .github/workflows/ci.yml

key-decisions:
  - "Host on the high-level pmcp::Server (not a raw ServerCore-over-pump): it carries .task_store() for the real lifecycle AND Server::run wires the peer needed by SamplingSource — both in one served instance. Satisfies the task_store/with_task_support acceptance and resolves the ServerCore-has-no-peer tension for the hosted-sampled path."
  - "SamplingSource is native-only, gated off wasm32, because it rides pmcp::PeerHandle which is itself #[cfg(not(wasm32))]. The wasm32 gate honestly proves the LOOP + SEAMS + config path is target-clean, NOT SamplingSource/adapter-on-wasm (D-13, honestly scoped)."
  - "Added three Arc<dyn Seam> blanket impls so the generic engine can be parameterised over an erased per-request source + shared seams without the adapter naming a concrete source type."
  - "No new contract YAML: pmcp-agent is an experimental 0.x crate with a trait-based extension surface and no external conformance clients this phase; the adapter's forward composition target is already covered by contracts/team-servers-v1.yaml (Phase 109). make quality-gate / pmat comply check still run against existing contracts."

patterns-established:
  - "CompletionSourceFactory: construct the completion source per request (peer-scoped sampling vs fixed HTTP)"
  - "Task-shaped return value as the synchronous-completion lifecycle mechanism (not orphan related_task metadata)"

requirements-completed: [AGNT-04, AGNT-05, AGNT-06, AGNT-07, AGNT-09]

# Metrics
duration: 95min
completed: 2026-07-18
---

# Phase 108 Plan 06: Agent-as-Server Adapter + Same-Loop Proof + wasm32 Gate + D-09 Mapping Summary

**Completed the phase: an agent is exposed as an MCP server through a package-driven, task-supported tool with a REAL store-backed task lifecycle and per-request sampling; the same engine is proven standalone and hosted-sampled; the loop/seam path is gated wasm-clean in CI; and a D-09 shape-compatibility mapping shows the durable Lambda could adopt the crate and delete code.**

## Performance

- **Duration:** ~95 min
- **Started:** 2026-07-17T (wave 4 spawn)
- **Completed:** 2026-07-18
- **Tasks:** 3 completed
- **Files created/modified:** 14

## Accomplishments

- **AGNT-07 / D-10/11/12 adapter.** `AgentServer` builds a single-tool
  `pmcp::Server` whose tool is `with_task_support(TaskSupport::Required)` and
  backed by an `InMemoryTaskStore` — a task-augmented `tools/call` mints a
  store-backed task, runs the agent synchronously, and persists the terminal
  `CallToolResult`, observable via `tasks/get` + `tasks/result` (a REAL
  create → completed lifecycle, NOT orphan `related_task` metadata). The tool
  name is the package name; its description is DERIVED (`derive_tool_description`,
  since `AgentPackage` has no `description` field); its input schema carries an
  optional `run_id`/`conversation_id` for resume; the completion source is built
  PER request via a `CompletionSourceFactory` from `extra.peer()`.
- **Same loop, two sources (AGNT-04/05/06).** `examples/s50_standalone_vs_sampled`
  runs one `AgentEngine` two ways — standalone over a mock source and hosted via
  `AgentServer` sampled through a real `Client` (`on_sampling_with_tools`) — both
  network-free under DEFAULT features. Anthropic is covered at compile level;
  live Ollama is opt-in behind an env flag. `tests/real_loop_sampling` drives the
  FULL engine over `SamplingSource` on a real `Server::run` to `Completed` (D-03).
- **AGNT-09 end-to-end.** `tests/e2e_package_to_adapter` composes
  `AgentPackage → resolve_agent → ResolvedAgentConfig → (mock source +
  ClientToolInvoker over a mock connector + AgentEngine) → AgentServer` and drives
  one `tools/call` to a terminal, store-backed task result.
- **wasm32 gate (D-13).** `ci.yml` gains `pmcp-agent-targets`: a DEFAULT-feature
  `wasm32-unknown-unknown` build (loop + seams + config, no reqwest/StreamableHttp)
  plus a native `openai-compat,anthropic,url-connector` build, wired into the
  required `gate` job.
- **D-09 mapping.** `docs/design/pmcp-agent-vs-durable-lambda-mapping.md` maps each
  `iteration.rs`/`mcp/client.rs`/`llm/` piece to its `pmcp-agent` counterpart and
  states which reference code becomes deletable — with NO private code copied.

## Task Commits

1. **Task 1: Agent-as-server adapter with a real store-backed task lifecycle + CompletionSourceFactory** — `43f2554d` (feat)
2. **Task 2: Example + full-loop D-03 test + AGNT-09 e2e + wasm32 CI gate** — `37e03ec0` (feat)
3. **Task 3: D-09 shape-compatibility mapping artifact** — `36a25016` (docs)

_Task 1 also carries the enabling seam work (Arc<dyn Seam> blanket impls, SamplingSource native-only gating) as a single cohesive unit — see TDD note below._

## Files Created/Modified

- `crates/pmcp-agent/src/adapter/server.rs` — `AgentServer`/`AgentServerBuilder`, `derive_tool_description`, input-schema builder, the task-shaped tool handler, and `outcome_to_result` (native-only).
- `crates/pmcp-agent/src/adapter/factory.rs` — `CompletionSourceFactory` seam + `SamplingSourceFactory` (from `extra.peer()`) + `FixedSourceFactory` (native-only).
- `crates/pmcp-agent/src/adapter/mod.rs` — native-only submodule declarations + re-exports.
- `crates/pmcp-agent/src/lib.rs` — native-only top-level adapter re-exports; added `TurnMessage`.
- `crates/pmcp-agent/src/seams/{completion,tool,store}.rs` — `Arc<dyn Seam>` blanket impls (wasm-clean).
- `crates/pmcp-agent/src/sources/mod.rs` — gated `SamplingSource` native-only (rides native-only `pmcp::PeerHandle`).
- `crates/pmcp-agent/tests/adapter_agent_as_server.rs` — real lifecycle + run_id resume + fresh-run independence.
- `crates/pmcp-agent/tests/real_loop_sampling.rs` — full engine over SamplingSource → Completed.
- `crates/pmcp-agent/tests/e2e_package_to_adapter.rs` — AGNT-09 package→adapter round-trip.
- `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs` — same loop, two sources.
- `docs/design/pmcp-agent-vs-durable-lambda-mapping.md` — D-09 mapping.
- `.github/workflows/ci.yml` — `pmcp-agent-targets` job + `gate` wiring.

## Deviations from Plan

### 1. [Rule 1 - Bug] SamplingSource is native-only (the plan's wasm claim was impossible)

- **Found during:** Task 2 (adding the wasm32 gate).
- **Issue:** The plan's must_have said the wasm32 gate proves "zero-dep
  SamplingSource" compiles target-clean. Empirically, `pmcp::PeerHandle` is
  declared `#![cfg(not(target_arch = "wasm32"))]`, and `SamplingSource` is built
  over it, so `cargo build --target wasm32-unknown-unknown` failed with a single
  `unresolved import pmcp::PeerHandle`. SamplingSource CANNOT be wasm-clean.
- **Fix:** Gated `sources::sampling` (and its re-export) native-only. The wasm32
  gate now honestly proves the LOOP + SEAMS + config path is target-clean; the
  adapter and SamplingSource are `cfg`-excluded on wasm (D-13, "honestly scoped").
  Updated the module docs + CI comment to say so.
- **Files modified:** `crates/pmcp-agent/src/sources/mod.rs`, `.github/workflows/ci.yml`.
- **Commits:** `43f2554d`, `37e03ec0`.

### 2. [Architectural clarification, no user decision needed] Host on the high-level `pmcp::Server`, not a raw `ServerCore`-over-pump

- **Context:** The plan's interfaces referenced `ServerCoreBuilder::task_store` +
  `with_task_support` (verified via the s45 analog, which serves a `ServerCore`
  through a simple duplex pump with NO peer). But the phase ALSO requires the
  hosted-sampled path (s50), where the adapter's tool must call `extra.peer()` —
  and a `ServerCore`-over-pump has no peer wiring (that needs a
  `ServerRequestDispatcher` + transport actor).
- **Resolution (not a Rule 4 stop):** The high-level `pmcp::Server` now carries
  `.task_store(Arc<dyn TaskStore>)` (`src/server/mod.rs:4143`) AND its `run()`
  wires the request-scoped peer (`:997`) AND its create-path runs the SAME
  store-backed `maybe_build_task_created` gate as `ServerCore` (`:1652`). Building
  on it gives the REAL task lifecycle AND sampling in ONE served instance. This
  satisfies the enforceable acceptance criteria verbatim (`grep task_store` ✓,
  `grep with_task_support` ✓, `with_task_store` NOT used ✓, adapter `cfg(not
  wasm32)` ✓, `derive_tool_description` ✓) and honors the intent ("an agent
  exposed as an MCP server with a real store-backed task lifecycle"). It is not
  an architectural change to the plan's data model — only the host object differs
  (a Server that internally uses the same task-dispatch unit), so no user decision
  was required.
- **Files:** `crates/pmcp-agent/src/adapter/server.rs`.
- **Commit:** `43f2554d`.

### 3. [Rule 3 - Supporting infra] `Arc<dyn Seam>` blanket impls

- **Found during:** Task 1.
- **Issue:** `AgentEngine<C,T,S>` takes each seam by value; the adapter builds a
  per-request `Arc<dyn CompletionSource>` and shares one invoker/store — none of
  which satisfied the seam traits by themselves.
- **Fix:** Added blanket impls of `CompletionSource`/`ToolInvoker`/
  `ConversationStore` for `Arc<dyn …>` (the `ToolInvoker` impl forwards
  `invoke_batch` so a concrete override, e.g. `ClientToolInvoker`'s bounded
  concurrency, is preserved). Small, idiomatic, wasm-clean.
- **Files modified:** `crates/pmcp-agent/src/seams/{completion,tool,store}.rs`.
- **Commit:** `43f2554d`.

## TDD Gate Compliance

Task 1 is marked `tdd="true"`. This crate's zero-defect policy (CLAUDE.md) means
every commit must build + clippy + test clean, so a separate RED commit of a
non-compiling test is not committable here. The behavior-driving test
(`adapter_agent_as_server.rs`) was written and run to observe failure locally,
then implemented to green and committed as ONE cohesive `feat(108-06)` unit
alongside the adapter. Git log therefore shows a single `feat` commit rather than
a `test`→`feat` pair; the tests are present and green in the same commit.

## Contract-First Note

No new contract YAML was added. `pmcp-agent` is an experimental 0.x crate with a
trait-based extension surface and no external conformance clients this phase; its
adapter's forward composition target (`team_mcp__<member>`) is already covered by
`contracts/team-servers-v1.yaml` (Phase 109 conformance). `make quality-gate` /
`pmat comply check` still run against existing contracts.

## Known Stubs

None. The `FixedSourceFactory`/`SamplingSourceFactory` and the mock sources in
tests are genuine seam implementations, not data stubs feeding a UI. The
`UnavailablePeerSource` fallback (returned when no peer is attached) is a
documented graceful-error path (transient error, not a panic), not a stub.

## Threat Flags

None. The adapter's new surface (client → agent tool input; agent run →
downstream connectors) is exactly the surface enumerated in the plan's
`<threat_model>` (T-108-06-01..SC); input is serde-typed against the package
schema, runs are bounded by `ResolvedAgentConfig` limits + a fresh-run-per-call
boundary, `run_id` is an opaque key namespaced to the adapter's own
`ConversationStore`, and errors are sanitized (no secrets in logs; the example
never prints keys).

## Verification

- `cargo test -p pmcp-agent -- --test-threads=1`: all pass (44 lib + adapter/real-loop/e2e/sampling/replay/fuzz/object-safety integration suites).
- `cargo test -p pmcp-agent --features openai-compat,anthropic,url-connector -- --test-threads=1`: all pass (74 lib + integration + 4 HTTP-mock).
- `cargo run -p pmcp-agent --example s50_standalone_vs_sampled`: runs to completion, NO network, NO feature flags.
- `cargo build -p pmcp-agent --target wasm32-unknown-unknown` (default features): succeeds.
- `cargo build -p pmcp-agent --features openai-compat,anthropic,url-connector` (native): succeeds.
- `cargo clippy -p pmcp-agent --all-targets --all-features -- -D warnings`: clean.
- `cargo fmt -p pmcp-agent --check`: clean. `cargo test -p pmcp-agent --doc …`: pass.
- `.github/workflows/ci.yml`: `wasm32-unknown-unknown` gate + native feature-build line present; wired into the required `gate`.

## Self-Check: PASSED

- Created files verified present: `adapter/{factory,server}.rs`, `tests/{adapter_agent_as_server,real_loop_sampling,e2e_package_to_adapter}.rs`, `examples/s50_standalone_vs_sampled.rs`, `docs/design/pmcp-agent-vs-durable-lambda-mapping.md`.
- Task commits verified in git log: `43f2554d` (Task 1), `37e03ec0` (Task 2), `36a25016` (Task 3).
- No file deletions introduced across the three commits.

---
*Phase: 108-pmcp-agent-loop-crate*
*Completed: 2026-07-18*
