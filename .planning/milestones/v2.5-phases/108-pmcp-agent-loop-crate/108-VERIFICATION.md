---
phase: 108-pmcp-agent-loop-crate
verified: 2026-07-18T17:55:08Z
status: passed
score: 9/9 must-haves verified (AGNT-07 wasm-adapter scope accepted by developer 2026-07-18, matching locked decision D-13)
overrides_applied: 1
resolution: "AGNT-07 scope decision RESOLVED 2026-07-18 — developer accepted the native-only adapter as intended scope (matches locked D-13: wasm32 gate proves loop+seams+config target-cleanliness; adapter/SamplingSource native-only; per-target deploy demos are Phase 110/111). ROADMAP SC#4 wording reconciled to match. Adapter-on-wasm, if ever needed, is a separate pmcp-core effort."
gaps: []
human_verification:
  - test: "Accept or reject the AGNT-07 WASM-adapter scope narrowing (D-13) — RESOLVED: ACCEPTED 2026-07-18"
    expected: "ROADMAP.md Phase 108 Success Criterion #4 literally reads 'deployable through existing Lambda/Docker/WASM target adapters.' The shipped adapter (`crates/pmcp-agent/src/adapter/`) is native-only (`#[cfg(not(target_arch = \"wasm32\"))]`) because it depends on `pmcp::Server::task_store` and `pmcp::PeerHandle`, both native-only in the SDK today. Only the loop+seams+config path (not the adapter) is proven wasm32-clean by the CI gate. A developer/product-owner must decide whether this narrower, pre-planned scope (documented in 108-CONTEXT.md D-13 before execution began) satisfies the roadmap intent, or whether ROADMAP.md's SC #4 needs updating / a follow-up phase is required for a wasm-capable adapter."
    why_human: "This is a scope-interpretation call the codebase cannot resolve programmatically — verified empirically (wasm32 build of the adapter module fails without the native-only gate; confirmed via `cfg` inspection and successful `cargo build -p pmcp-agent --target wasm32-unknown-unknown` which only compiles because the adapter/sampling modules are excluded). Lambda and Docker deployability of the adapter IS demonstrated (it hosts on `pmcp::Server::run<T: Transport>`, the same generic transport surface cargo-pmcp's Lambda/Docker templates use)."
---

# Phase 108: `pmcp-agent` Loop Crate Verification Report

**Phase Goal:** The agent runtime ships as an open, deploy-anywhere `crates/pmcp-agent` (0.x, experimental, isolated from `pmcp` core) — a pure decision loop between object-safe effect seams, three CompletionSources (sampling-first), an agent-as-server adapter, and a tasks-aware ToolInvoker, all configured from an `AgentPackage`. pmcp.run's `handler/iteration.rs` becomes a platform-specific composition of this loop.

**Verified:** 2026-07-18T17:55:08Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (AGNT-01..09, mapped from ROADMAP Success Criteria + REQUIREMENTS.md)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | AGNT-01: object-safe async `CompletionSource`/`ToolInvoker`/`ConversationStore` seams; `CompletionSource` reuses SDK sampling types verbatim | VERIFIED | `crates/pmcp-agent/src/seams/{completion,tool,store}.rs` — three `#[async_trait]` traits, all `Send + Sync`, no generics; `CompletionSource::create_message` returns `pmcp::types::sampling::CreateMessageResultWithTools` (SDK type, not a local mirror). `tests/object_safety.rs::all_three_seams_are_object_safe` constructs `Arc<dyn CompletionSource>`, `Arc<dyn ToolInvoker>`, `Arc<dyn ConversationStore>` and compiles/passes. |
| 2 | AGNT-02: iteration loop pure between seams; retry classification exposed as data (`RunOutcome::RetryRequired{class}`), no retry/backoff policy inside the loop | VERIFIED | `crates/pmcp-agent/src/iteration/decide.rs` — every function synchronous, no `std::time`/RNG (grep confirms zero hits outside doc comments and an unrelated HTTP-client-timeout `Duration` in `sources/http_common.rs`). `crates/pmcp-agent/src/iteration/engine.rs::step` awaits ONLY `create_message`/`invoke_batch`/`save`; `RunOutcome::RetryRequired{class: RetryClass}` returned as data, never slept on (`retry_or_fail` maps class → outcome, no `sleep`/`backoff` anywhere in the crate). |
| 3 | AGNT-03: replay-safety property-tested over recorded `EffectTrace` — identical effect results ⇒ identical loop DECISIONS | VERIFIED | `crates/pmcp-agent/tests/replay_safety.rs::identical_trace_yields_identical_decision_sequence` — a proptest running the engine TWICE over one generated `EffectTrace` via `ReplaySource`/`ReplayInvoker`, asserting full `DecisionTrace` (step-by-step decisions, not just final outcome) equality. Two golden fixtures (`tests/fixtures/golden_trace_{end_turn,tool_loop}.json`) pin exact decision sequences. Ran locally: passes. |
| 4 | AGNT-04: `SamplingSource` (zero-dep spec sampling incl. tools/tool_choice) runs the loop, proven on the real `Server::run` | VERIFIED | `crates/pmcp-agent/src/sources/sampling.rs` wraps `Arc<dyn pmcp::PeerHandle>`, forwards to `PeerHandle::sample_with_tools` — zero new deps. `tests/real_loop_sampling.rs::full_engine_over_sampling_source_reaches_completed` drives the FULL `AgentEngine` inside a real `ToolHandler` on a real `pmcp::Server::run` + real `pmcp::Client` (duplex transport), tool-call then end-turn, asserting `RunOutcome::Completed` and exactly 1 tool dispatch. This rides the 108-01 D-106-A Transport Actor fix. Ran locally: passes. |
| 5 | AGNT-05: feature-gated `OpenAiCompatSource` | VERIFIED | `crates/pmcp-agent/src/sources/openai_compat.rs`, gated `#[cfg(feature = "openai-compat")]`, non-default. Implements `/chat/completions` request/response mapping (system hoist, tools, tool_choice, malformed-args → `Decode` not panic). `tests/http_sources_mock.rs` exercises it against a dependency-free raw-TCP HTTP mock (asserts path, `Authorization: Bearer`, body, 5xx, timeout). Ran locally with `--features openai-compat,anthropic`: passes (4 mock tests). |
| 6 | AGNT-06: feature-gated `AnthropicSource` | VERIFIED | `crates/pmcp-agent/src/sources/anthropic.rs`, gated `#[cfg(feature = "anthropic")]`. Implements `/v1/messages` with pure `normalize_history` (system hoist, tool_result→user, consecutive-same-role merge for parallel tool calls) and `x-api-key`/`anthropic-version` headers. Same mock harness proves wire shape and no auth-header leak. |
| 7 | AGNT-07: agent exposed as MCP server via a `ServerCore`/`Server` adapter (deployable), wasm32 default-feature compile gate | **PARTIAL — see human decision above** | `crates/pmcp-agent/src/adapter/server.rs::AgentServer` builds a real single-tool `pmcp::Server` with `.task_store(Arc<dyn TaskStore>)` + `with_task_support(TaskSupport::Required)` — a genuine create→working→completed task lifecycle (not orphan `related_task` metadata), proven end-to-end by `tests/adapter_agent_as_server.rs` (real `tasks/get`/`tasks/result` polling, resume via `run_id`, independent fresh runs) and `tests/e2e_package_to_adapter.rs`. **However**, the adapter module and `SamplingSource` are `#[cfg(not(target_arch = "wasm32"))]` — confirmed by reading `adapter/mod.rs`/`sources/mod.rs` and by `cargo build -p pmcp-agent --target wasm32-unknown-unknown` (default features) succeeding ONLY because those modules are excluded from that build. The CI `pmcp-agent-targets` job (`.github/workflows/ci.yml:334-361`, wired into the required `gate`) builds wasm32 with DEFAULT features (loop+seams+config only) and natively with `openai-compat,anthropic,url-connector` — it never builds the adapter for wasm32. ROADMAP.md's literal SC #4 text ("deployable through existing Lambda/Docker/WASM target adapters") is therefore not fully met for WASM; Lambda/Docker ARE met (`AgentServer::run<T: Transport>` is the same generic-transport surface cargo-pmcp's Lambda/Docker templates use). This was a **pre-planned, documented decision (108-CONTEXT.md D-13, refined during --reviews replan BEFORE execution)**, not a shortcut discovered after the fact — see 108-06-PLAN.md `must_haves` (already narrowed to "native-only … existing native target adapters") and 108-06-SUMMARY.md Deviation #1. |
| 8 | AGNT-08: `ToolInvoker` over `pmcp::Client` honors task-augmented tool results via `poll_decision`/`wait_for_related_task` (SEP-1686) | VERIFIED | `crates/pmcp-agent/src/invoker/client.rs::ClientToolInvoker::dispatch` — on `result.related_task()` present, drives to terminal via `ConnectorClient::wait_for_related_task` under a HOST-CONFIGURED HARD `max_poll_duration_secs` cap (never unbounded). `invoke_batch` overrides the sequential default with `buffered(N)` bounded-concurrency dispatch, input-order/id-matched. `tests/invoker_task_augmented.rs` proves: task-augmented results resolve to final value under the cap; a never-completing task returns a timeout `ToolCallResult` (never hangs, never panics); `invoke_batch` peak concurrency is bounded at exactly N with real overlap. Ran locally: 5 tests pass. |
| 9 | AGNT-09: agent fully configured from an `AgentPackage` + resolved config slots (`resolve_agent` → `ResolvedAgentConfig`) | VERIFIED | `crates/pmcp-agent/src/config/resolver.rs::resolve_agent` composes `AgentPackage` → LLM slot resolution (warn-and-run on `tested_value` deviation per D-15, `RedactedSecret` for identity-bearing slots per ASVS V7) → flattened tool selection → clamped integer limits → I/O schemas → connector endpoint map → `ResolvedAgentConfig`. `SlotResolver` seam has two impls (`EnvVarResolver`, `ProgrammaticBuilder`) per D-14. `tests/e2e_package_to_adapter.rs::package_resolves_and_composes_through_the_adapter` drives the FULL chain (`AgentPackage → resolve_agent → ResolvedAgentConfig → mock source + ClientToolInvoker(mock) + AgentEngine → AgentServer`) to a terminal, store-backed task result over a real client. Ran locally: passes. |

**Score:** 8/9 truths fully VERIFIED; 1/9 (AGNT-07) is substantively delivered but contains a literal scope gap against the ROADMAP's WASM-deployability wording that requires a developer decision (accept via override, or treat as a tracked follow-up).

### D-106-A Core Fix (paired `pmcp` requirement, enables AGNT-04)

| Item | Status | Evidence |
|------|--------|----------|
| `Server::run` no longer deadlocks on in-tool `peer.sample()`/`.list_roots()` | VERIFIED | `src/server/mod.rs` rewritten to a single-owner Transport Actor (`run_transport_actor`, unbounded worker queue, responses routed immediately). `tests/in_tool_peer_roundtrip.rs` — 5 tests, run locally, all pass. |
| Real-loop proof standard (D-03): tests run through the REAL `Server::run` + real `Client`, no raw pump | VERIFIED | Confirmed in `tests/in_tool_peer_roundtrip.rs`, `crates/pmcp-agent/tests/real_loop_sampling.rs`, `crates/pmcp-agent/tests/adapter_agent_as_server.rs` — all use `Server::builder()...build()` + `server.run(transport)` + `Client::new`/`ClientBuilder`, not a raw duplex pump bypassing the dispatcher. |
| pmcp minor bump paired with `pmcp-agent` 0.1.0 | VERIFIED | Root `Cargo.toml` `version = "2.17.0"`; `CHANGELOG.md` has a `## [2.17.0] - Unreleased` entry documenting the Transport Actor + WithTools sampling surface; `cargo-pmcp/src/templates/workbook_server.rs` `PMCP_VERSION = "2.17.0"` (scaffold-pin tripwire updated, matches root). |
| Root `pmcp` lib test suite regression-free after the Transport Actor rewrite | VERIFIED | `cargo test --lib -- --test-threads=1`: 908 passed, 0 failed. |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-agent/Cargo.toml` | New 0.x experimental workspace member, pins `pmcp = "2.17.0"`, `pmcp-package = "0.1"`, `openai-compat`/`anthropic`/`url-connector` non-default | VERIFIED | Confirmed by reading the manifest; registered in root `Cargo.toml` `[workspace].members`; `crates/pmcp-package` correctly added to `[workspace].exclude` to resolve the multi-root conflict. |
| `crates/pmcp-agent/src/seams/{completion,tool,store}.rs` | Three object-safe seams | VERIFIED | Read in full; `Arc<dyn T>` blanket impls present for engine composability. |
| `crates/pmcp-agent/src/iteration/{decide,engine,result}.rs` | Pure decision core + thin async engine | VERIFIED | Read in full; crash-safe checkpoint ordering (`PendingTools` saved BEFORE dispatch) verified both by code inspection and by `engine.rs::tests::saves_pending_tools_before_dispatch` / `resumes_pending_tools_without_rerunning_completion`. |
| `crates/pmcp-agent/src/trace.rs` | Public `EffectTrace`/`DecisionTrace` replay substrate | VERIFIED | Read in full; `#[derive(Serialize, Deserialize)]`, `preserve_order` json, no floats. |
| `crates/pmcp-agent/src/sources/{sampling,openai_compat,anthropic,http_common,secret}.rs` | Three CompletionSources + shared hardening | VERIFIED | Read in full; `SecretString`/`RedactedSecret` redact `Debug`/`Display`; `http_common` centralizes endpoint-scheme policy, timeout, bounded-body read, status→RetryClass. |
| `crates/pmcp-agent/src/invoker/{client,factory}.rs` | Tasks-aware `ClientToolInvoker` + `ConnectorClientFactory` | VERIFIED | Read in full; `url-connector` feature correctly isolates the native-only `StreamableHttpTransport` reference from the default/wasm build. |
| `crates/pmcp-agent/src/config/{resolver,endpoint,mod}.rs` | `SlotResolver` + `resolve_agent` | VERIFIED | Read in full. |
| `crates/pmcp-agent/src/adapter/{server,factory,mod}.rs` | Agent-as-server adapter (native-only) | VERIFIED (native scope only — see AGNT-07 above) | Read in full. |
| `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs` | Same loop, two sources, network-free | VERIFIED | Ran: `cargo run -p pmcp-agent --example s50_standalone_vs_sampled` — completes both the standalone and hosted-sampled paths with no network access, default features. |
| `docs/design/pmcp-agent-vs-durable-lambda-mapping.md` | D-09 shape-compatibility mapping, no private code copied | VERIFIED | Read in full — a genuine mapping table (durable-lambda piece → shape → `pmcp-agent` counterpart) with explicit "what gets deleted" and "what stays platform-side" sections; no reproduced private-repo source. |
| `.github/workflows/ci.yml` `pmcp-agent-targets` job | wasm32 default-feature build + native full-feature build, wired into required `gate` | VERIFIED | Read in full; `gate` job's `needs:` array includes `pmcp-agent-targets` and its pass/fail check includes `AGENT_TARGETS_RESULT`. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `AgentEngine::step` | `CompletionSource::create_message` / `ToolInvoker::invoke_batch` / `ConversationStore::save` | direct awaits, no other side effects between them | WIRED | Confirmed by reading `engine.rs`; the only `.await` points in `step`/`resume_pending` are these four seam calls. |
| `adapter/server.rs::run_agent_tool` | `iteration::AgentEngine` | constructs one engine per tool call over the per-request source + shared invoker/store, `.run(&run_id)` | WIRED | `43f2554d` commit + `tests/adapter_agent_as_server.rs` proves a real `tools/call` → engine run → task-store-persisted result round trip. |
| `adapter/server.rs` | `pmcp::Server::task_store` + `with_task_support(TaskSupport::Required)` | real store-backed task lifecycle (create→completed), not orphan `related_task` | WIRED | Grep-confirmed (`task_store`/`with_task_support` present); `tests/adapter_agent_as_server.rs` polls `tasks/get`→terminal and reads `tasks/result` with persisted content. |
| `invoker/client.rs::ClientToolInvoker::dispatch` | `ConnectorClient::wait_for_related_task` | hard `max_poll_duration_secs` cap set on `WaitForTaskOptions`, `Some(cap)` wins over metadata hints | WIRED | `tests/invoker_task_augmented.rs` asserts the mock always observes a set cap and that a never-completing task returns a bounded timeout, not a hang. |
| `sources/sampling.rs::SamplingSource` | `pmcp::PeerHandle::sample_with_tools` | direct forward + error mapping | WIRED | `tests/real_loop_sampling.rs` proves this end-to-end through a real `Server::run`. |
| `config/resolver.rs::resolve_agent` | `adapter::AgentServer` | via `ResolvedAgentConfig` | WIRED | `tests/e2e_package_to_adapter.rs` composes the full chain and drives a live `tools/call`. |

### Data-Flow Trace (Level 4)

Not applicable in the UI-rendering sense (this is a library crate, not a rendering surface). The equivalent check — decision determinism under real recorded data — is covered by AGNT-03's replay-safety property (Truth #3) and the golden fixtures, which are genuinely generated/consumed data (not hardcoded empty stand-ins): `EffectTrace` fixtures carry real `CreateMessageResultWithTools` payloads and are asserted to produce non-trivial `DecisionTrace`s (`decisions.steps.len() == 1` / `== 2` with concrete `tool_call_ids`).

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| pmcp-agent test suite (all features) | `cargo test -p pmcp-agent --all-features -- --test-threads=1` | 107 passed (12 suites) | PASS |
| wasm32 default-feature build | `cargo build -p pmcp-agent --target wasm32-unknown-unknown` | Success (0 crates recompiled from cache; verified from clean by CI job definition) | PASS |
| Native full-feature build | `cargo build -p pmcp-agent --features openai-compat,anthropic,url-connector` | Success | PASS |
| s50 example (same loop, two sources) | `cargo run -p pmcp-agent --example s50_standalone_vs_sampled` | Completes standalone + hosted-sampled paths, no network | PASS |
| D-106-A real-loop proof (pmcp core) | `cargo test -p pmcp --test in_tool_peer_roundtrip -- --test-threads=1` | 5 passed | PASS |
| pmcp-agent clippy (all targets, all features) | `cargo clippy -p pmcp-agent --all-targets --all-features -- -D warnings` | No issues found | PASS |
| pmcp core lib regression check | `cargo test --lib -- --test-threads=1` | 908 passed, 0 failed | PASS |
| HTTP-source mock suite | `cargo test -p pmcp-agent --features openai-compat,anthropic --test http_sources_mock -- --test-threads=1` | 4 passed | PASS |
| Config resolver suite | `cargo test -p pmcp-agent --test config_resolver -- --test-threads=1` | 8 passed | PASS |

### Probe Execution

No `scripts/*/tests/probe-*.sh` probes declared or discovered for this phase (not a migration/tooling phase in that sense). Skipped — N/A.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| AGNT-01 | 108-02 | Object-safe seams, verbatim SDK sampling types | SATISFIED | `seams/*.rs`, `tests/object_safety.rs` |
| AGNT-02 | 108-03 | Pure loop, retry-as-data | SATISFIED | `iteration/decide.rs`, `iteration/engine.rs`, `iteration/result.rs::RunOutcome` |
| AGNT-03 | 108-03 | Replay-safety property test | SATISFIED | `tests/replay_safety.rs` + golden fixtures |
| AGNT-04 | 108-01, 108-04, 108-06 | `SamplingSource` on real `Server::run` | SATISFIED | `sources/sampling.rs`, `tests/real_loop_sampling.rs`, `examples/s50_standalone_vs_sampled.rs` |
| AGNT-05 | 108-04 | `OpenAiCompatSource` | SATISFIED | `sources/openai_compat.rs`, `tests/http_sources_mock.rs` |
| AGNT-06 | 108-04 | `AnthropicSource` | SATISFIED | `sources/anthropic.rs`, `tests/http_sources_mock.rs` |
| AGNT-07 | 108-06 | Agent-as-server, deployable via existing target adapters | PARTIALLY SATISFIED (native-only; WASM-adapter gap flagged for human decision) | `adapter/server.rs`, `tests/adapter_agent_as_server.rs`; wasm32 gate proves loop/seams/config only |
| AGNT-08 | 108-05 | Tasks-aware `ToolInvoker` (SEP-1686) | SATISFIED | `invoker/client.rs`, `tests/invoker_task_augmented.rs` |
| AGNT-09 | 108-05 | `AgentPackage` → `ResolvedAgentConfig` | SATISFIED | `config/resolver.rs`, `tests/e2e_package_to_adapter.rs` |

**Note (process, informational only):** `.planning/REQUIREMENTS.md`'s coverage table still lists AGNT-01..09 as "Pending" against Phase 108 even though ROADMAP.md marks the phase complete (2026-07-18) and the code fully backs 8/9 of them. This is a documentation-sync gap in REQUIREMENTS.md, not a code gap — flagged here so it can be updated alongside closing this verification, but it is not a blocker.

**No orphaned requirements found:** every AGNT-0x id referenced in REQUIREMENTS.md for Phase 108 is claimed by one of the six plans' `requirements` frontmatter fields.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/pmcp-agent/src/sources/openai_compat.rs` | 193 | Comment: "forward a placeholder so history stays well-formed" (non-text modality forwarding) | INFO | Documented, intentional behavior (not a data stub feeding the user) — non-text content blocks (image/audio) are forwarded as well-formed placeholders so conversation history stays schema-valid; streaming/rich multimodal request bodies are explicitly out of scope this phase per 108-04-SUMMARY.md. Does not affect any of the 9 requirements. |

No `TBD`/`FIXME`/`XXX` markers found in any file touched by this phase. No `TODO`/`HACK` markers found. No empty `return null`/`return Ok(())`-only stub implementations found in the reviewed seam/engine/source/invoker/adapter files — every implementation reviewed carries real logic backed by passing tests.

### Human Verification Required

### 1. Accept or reject the AGNT-07 WASM-adapter scope narrowing (D-13)

**Test:** Review whether the phase's documented, pre-planned decision to make the agent-as-server adapter native-only (not wasm32-deployable) satisfies the intent of ROADMAP.md Phase 108 Success Criterion #4 ("An agent is exposed as an MCP server via a `ServerCore` adapter (deployable through existing Lambda/Docker/WASM target adapters)...").

**Expected:** Either (a) accept the narrowing — the underlying SDK primitives the adapter needs (`pmcp::Server::task_store`, `pmcp::PeerHandle`) are themselves native-only today, so a wasm-deployable *hosted* agent is not achievable without separate SDK-core wasm work outside this phase's scope, and Lambda/Docker deployability IS demonstrated (`AgentServer::run<T: Transport>` uses the same generic-transport pattern as existing deploy templates) — and add a `overrides:` entry to this file; or (b) treat this as a tracked gap for a follow-up phase (e.g. Phase 110/111 or a new wasm-adapter phase) and update ROADMAP.md SC #4 wording to match reality.

**Why human:** This is a scope-interpretation/acceptance call, not a code-correctness question. The technical facts are settled and verified (wasm32 build only succeeds because the adapter and `SamplingSource` modules are `cfg`-excluded; this was decided in `108-CONTEXT.md` D-13 before planning/execution began, and the 108-06 PLAN's own `must_haves` already reflect the narrower wording). Only a human/product owner can decide whether the roadmap's literal wording should be honored as-is or updated to match the phase's actual, well-justified scope.

To accept, add to this file's frontmatter:

```yaml
overrides:
  - must_have: "An agent exposed as an MCP server is deployable through existing Lambda/Docker/WASM target adapters"
    reason: "The agent-as-server adapter needs pmcp::Server::task_store and pmcp::PeerHandle, both native-only in the shipped SDK; a wasm-deployable hosted agent requires SDK-core wasm work out of this phase's scope. Lambda/Docker deployability is demonstrated (AgentServer::run<T: Transport> uses the same generic-transport pattern as existing deploy templates); the wasm32 CI gate honestly proves only the loop/seams/config path is target-clean, per 108-CONTEXT.md D-13 (locked before execution)."
    accepted_by: "<name>"
    accepted_at: "<ISO timestamp>"
```

### Gaps Summary

Eight of nine AGNT requirements are fully and robustly delivered with real, passing, end-to-end tests — including genuine proofs through the actual `pmcp::Server::run`/`Client` stack (not mocked shortcuts), a real proptest-driven replay-safety property, real bounded-concurrency and hard-capped task polling, and a real store-backed task lifecycle. The D-106-A paired core fix (`pmcp` 2.17.0) is verified both by code inspection and by running its dedicated real-loop test suite plus the full 908-test root lib suite (no regressions).

The single item requiring a decision (AGNT-07) is not a code defect: the crate delivers everything the plan promised (`108-06-PLAN.md`'s `must_haves` already state "native-only … existing native target adapters"), but that plan-level wording is narrower than ROADMAP.md's phase-level Success Criterion #4, and the roadmap text itself was not updated to match ("Roadmap unchanged" per 108-CONTEXT.md D-13). This is exactly the kind of pre-planned, well-justified, and thoroughly documented deviation the override mechanism exists for — surfaced here for a developer decision rather than silently passed or silently failed.

---

*Verified: 2026-07-18T17:55:08Z*
*Verifier: Claude (gsd-verifier)*
