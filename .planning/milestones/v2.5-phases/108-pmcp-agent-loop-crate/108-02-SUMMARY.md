---
phase: 108-pmcp-agent-loop-crate
plan: 02
subsystem: api
tags: [crate-scaffold, seams, object-safety, retry-class, run-state, mcp-2025-11-25]

# Dependency graph
requires:
  - phase: 108-pmcp-agent-loop-crate (plan 01)
    provides: pmcp 2.17.0 (CreateMessageResultWithTools, SamplingMessage, the WithTools sampling surface these seams are typed against)
  - phase: 107 (pmcp-package)
    provides: AgentPackage fields (the source shape for ResolvedAgentConfig; no description field)
provides:
  - "crates/pmcp-agent 0.1.0 — new experimental 0.x workspace member (mirrors pmcp-tasks isolation), pins pmcp 2.17 + pmcp-package 0.1"
  - "Three object-safe async effect seams: CompletionSource, ToolInvoker, ConversationStore (Arc<dyn>-safe, AGNT-01)"
  - "CompletionSource::create_message -> CreateMessageResultWithTools (reuses SDK sampling types verbatim)"
  - "ToolCall{id,name,arguments,connector} + ToolCallResult{id,..} id correlation; ToolInvoker::invoke_batch default sequential (one-per-input/ordered/id-matched contract, D-07)"
  - "Shared #[non_exhaustive] RetryClass data enum + per-seam retry_class() accessors (no backoff policy, no secrets)"
  - "RunState (SamplingMessage history + iteration/tokens_used counters + pending_tool_calls + RunPhase checkpoint, no floats/time) + InMemoryStore (std::sync::Mutex)"
  - "ResolvedAgentConfig runtime-config contract (instructions/tools/limits/schemas/model/endpoints)"
  - "non-default features openai-compat/anthropic/url-connector — default+wasm build is reqwest-free"
affects: [plan-108-03-iteration, plan-108-04-sources, plan-108-05-invoker-config, plan-108-06-adapter, AGNT-01]

# Tech tracking
tech-stack:
  added:
    - "crates/pmcp-agent: jsonschema 0.46 (default-features=false), uuid 1.17, thiserror 2.0, async-trait, futures, tracing; reqwest 0.13 (optional, feature-gated)"
  patterns:
    - "Classification-as-data RetryClass mirrors pmcp Task::poll_decision (non_exhaustive, exhaustive match)"
    - "Default trait method for invoke_batch (sequential over invoke) — SDK impl overrides with bounded parallelism in 108-05"
    - "std::sync::Mutex guard scoped to drop before any await (no async lock, no await_holding_lock)"
    - "Workspace-excluded path dep: crates/pmcp-package added to root [workspace].exclude so pmcp-agent (first in-repo consumer) does not trip cargo's multiple-workspace-roots error"

key-files:
  created:
    - crates/pmcp-agent/Cargo.toml
    - crates/pmcp-agent/src/lib.rs
    - crates/pmcp-agent/src/config/mod.rs
    - crates/pmcp-agent/src/seams/mod.rs
    - crates/pmcp-agent/src/seams/completion.rs
    - crates/pmcp-agent/src/seams/tool.rs
    - crates/pmcp-agent/src/seams/store.rs
    - crates/pmcp-agent/src/iteration/mod.rs
    - crates/pmcp-agent/src/sources/mod.rs
    - crates/pmcp-agent/src/invoker/mod.rs
    - crates/pmcp-agent/src/adapter/mod.rs
    - crates/pmcp-agent/src/trace.rs
    - crates/pmcp-agent/tests/object_safety.rs
  modified:
    - Cargo.toml
    - CLAUDE.md

requirements: [AGNT-01]

# Verification
verification:
  - "cargo build -p pmcp-agent (default features) succeeds; cargo tree -p pmcp-agent -e normal --no-default-features | grep -c reqwest == 0 (reqwest only via dev-dep pmcp full)"
  - "cargo test -p pmcp-agent -- --test-threads=1: 5 unit + 2 object_safety tests pass"
  - "object_safety.rs constructs Arc<dyn CompletionSource>, Arc<dyn ToolInvoker>, Arc<dyn ConversationStore>"
  - "invoke_batch default is one-per-input/input-order/id-matched (asserted)"
  - "cargo clippy -p pmcp-agent --all-targets: clean; cargo fmt -p pmcp-agent --check: clean"
---

# Plan 108-02 Summary: pmcp-agent crate scaffold + three effect seams

Scaffolded the experimental `crates/pmcp-agent` 0.x workspace member and defined the three
object-safe async effect seams (`CompletionSource`, `ToolInvoker`, `ConversationStore`) plus the
shared `RetryClass` and `ResolvedAgentConfig` contracts that all of Wave 3 builds against.

## Tasks

1. **Crate manifest, module tree, ResolvedAgentConfig** — new `pmcp-agent` 0.1.0 pinning
   `pmcp = 2.17` + `pmcp-package = 0.1`, with `openai-compat`/`anthropic`/`url-connector` all
   non-default (default + wasm32 build is reqwest-free). Registered the member + publish-order
   entry after `pmcp-package` (its first in-repo consumer). `ResolvedAgentConfig` binds
   instructions/tools/integer-limits/schemas/model/endpoint-map (no floats, no `description`).
2. **CompletionSource + ToolInvoker seams** — `CompletionSource` returns
   `CreateMessageResultWithTools`; `ToolCall`/`ToolCallResult` carry a correlating `id`;
   `invoke_batch` has a sequential default with a one-per-input/ordered/id-matched contract;
   shared `RetryClass` + per-seam `retry_class()` accessors carry no policy or secrets.
3. **ConversationStore + RunState/RunPhase + InMemoryStore** — resumable `RunState`
   (SamplingMessage history + counters + pending calls + checkpoint `RunPhase`, no floats/time)
   backed by `std::sync::Mutex`; `tests/object_safety.rs` proves all three seams object-safe.

## Deviations

- **Workspace-roots fix (integration):** `pmcp-agent` is the first in-repo crate to path-depend
  on the workspace-excluded `pmcp-package` (which has its own `[workspace]` table). That tripped
  cargo's "multiple workspace roots" error. Resolved by adding `crates/pmcp-package` to the root
  `[workspace].exclude` list — the minimal correct fix. Updated the CLAUDE.md publish-order note
  on `pmcp-package` accordingly (it now has a real consumer, so it must publish before
  `pmcp-agent`).
- **`RunState` has no `PartialEq`:** the SDK `SamplingMessage` in `history` does not implement it,
  so `RunState` derives `Debug, Clone, Serialize, Deserialize` only. Round-trip equality in tests
  is asserted via re-serialized JSON, which still proves lossless/deterministic (de)serialization.

## Self-Check: PASSED

- `cargo build -p pmcp-agent` (default) succeeds; library build pulls no reqwest (normal edges).
- `cargo test -p pmcp-agent -- --test-threads=1`: 5 unit + 2 object-safety tests pass.
- `cargo clippy -p pmcp-agent --all-targets` clean; `cargo fmt -p pmcp-agent --check` clean.
