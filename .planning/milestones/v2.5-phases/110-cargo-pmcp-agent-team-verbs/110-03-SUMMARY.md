---
phase: 110-cargo-pmcp-agent-team-verbs
plan: 03
subsystem: cargo-pmcp CLI
tags: [cli, cargo-pmcp, agent, agent-dev, completion-source, sampling, tdd]
requires:
  - pmcp-agent (0.1, Phase 108) — AgentEngine, OpenAiCompatSource, AgentServer, SamplingSourceFactory, resolve_agent
  - pmcp-package (0.1, Phase 107) — AgentPackage deserialization
  - 110-01 (foundation) — agent/dev.rs stub + DevArgs + async dispatch
provides:
  - cargo pmcp agent dev (CLI-02) wired for --source openai-compat|sampling|fixed
  - lib-safe run_fixed_source seam (cargo_pmcp::agent_run) reused by the 110-06 example
  - offline fixed-source + in-process sampling-hosted integration tests
affects:
  - cargo-pmcp/src/commands/agent/dev.rs
  - cargo-pmcp/src/commands/agent/run.rs
  - cargo-pmcp/src/commands/agent/mod.rs
  - cargo-pmcp/src/lib.rs
  - cargo-pmcp/tests/agent_dev.rs
tech-stack:
  added: []
  patterns:
    - clap ValueEnum (SourceKind) for --source dispatch
    - lib-safe leaf runner (no clap/GlobalFlags) mounted into the lib target via a #[path] seam (mirrors templates_agent)
    - correct pmcp-agent error/transport contract — Decode at construction, RunOutcome match after .run()
    - AgentPackage loaded from --package / ./agent.package.json / built-in demo (never a hardcoded fixture)
key-files:
  created:
    - cargo-pmcp/src/commands/agent/run.rs
    - cargo-pmcp/tests/agent_dev.rs
  modified:
    - cargo-pmcp/src/commands/agent/dev.rs
    - cargo-pmcp/src/commands/agent/mod.rs
    - cargo-pmcp/src/lib.rs
decisions:
  - "run_fixed_source is exposed to tests/examples as cargo_pmcp::agent_run (a #[path] seam in lib.rs), NOT cargo_pmcp::commands::agent::run — the commands::* tree is bin-only (Rule 3 deviation, follows the templates_agent convention)"
  - "openai-compat + sampling arms reuse the single pub NoopInvoker defined in run.rs (avoids a duplicate no-op invoker)"
  - "built-in demo package mirrors the agent new scaffold's starter_package shape so agent new → agent dev round-trips"
metrics:
  duration: 30min
  completed: "2026-07-19"
  tasks: 2
  files: 5
---

# Phase 110 Plan 03: cargo-pmcp `agent dev` (CLI-02) Summary

Wired `cargo pmcp agent dev` — the open agent loop, runnable locally two ways
from a real `AgentPackage` — filling the 110-01 stub. `--source` is now a clap
`ValueEnum` (`SourceKind { OpenaiCompat, Sampling, Fixed }`). The fixed path runs
offline through a lib-safe runner seam (`run_fixed_source`) that the CLI arm and
the plan-110-06 example both call; the openai-compat path drives `AgentEngine`
against `http://localhost:11434/v1` with the endpoint/error contract modelled
correctly; the sampling path serves an `AgentServer` over `pmcp::StdioTransport`.
Proven by two fully-offline, deterministic TDD tests.

## What Was Built

**Task 1 — RED tests** (`800f0300`)
- `cargo-pmcp/tests/agent_dev.rs`:
  - `fixed_source_runs_offline` — asserts a terminal `RunOutcome::Completed` from
    the lib seam AND asserts the REAL binary `agent dev --source fixed` exits 0.
  - `sampling_hosted_run_in_process` — mirrors the pmcp-agent s50 `run_hosted`
    shape: `AgentServer` + `SamplingSourceFactory` over an in-process
    `DuplexTransport`, a scripted `on_sampling_with_tools` host (immediate
    `end_turn`), driving `call_tool_with_task` to a terminal task status + a
    non-empty `tasks_result`. Inline `DuplexTransport` / `HostScript` /
    `NoopInvoker` (example-local shapes re-implemented).
  - Failed RED: `cargo_pmcp::agent_run` seam did not exist (compile error).

**Task 2 — GREEN implementation** (`cbff3b5c`)
- `commands/agent/run.rs` (new, lib-safe leaf): `pub async fn
  run_fixed_source(config: ResolvedAgentConfig) -> RunOutcome` over an inline
  end-turn `CompletionSource` + a shared `pub NoopInvoker` + `InMemoryStore`.
  References only `pmcp-agent` + `pmcp` types + std — NO `clap`/`GlobalFlags`.
- `lib.rs`: mounted `commands/agent/run.rs` as `pub mod agent_run` via a `#[path]`
  seam (mirrors `templates_agent`), so the lib target / tests / the 110-06 example
  reach `run_fixed_source` without the bin-only `commands::*` tree.
- `commands/agent/mod.rs`: `pub mod run;` (bin-target mount of the same file).
- `commands/agent/dev.rs`: replaced the stub.
  - `SourceKind` clap `ValueEnum` (default `OpenaiCompat`); added `--model`
    (default `llama3.2`); kept `endpoint`/`package`/`api_key_env`/
    `allow_insecure_http`.
  - `load_package`: `--package` → `./agent.package.json` → built-in demo;
    resolved via `resolve_agent(&pkg, &EnvVarResolver::new())`.
  - Fixed arm → delegates to `run_fixed_source`.
  - OpenaiCompat arm → `OpenAiCompatSource::with_options(endpoint, model, key,
    HttpSourceOptions { allow_insecure_http, ..Default })`; a construction
    `Err(CompletionError::Decode)` → bail naming `--allow-insecure-http`; then
    matches the returned `RunOutcome` (never tries to catch `CompletionError`
    from `.run()`) and bails actionably on a non-`Completed` outcome naming
    `--endpoint`/`--source fixed`.
  - Sampling arm → `AgentServer` over `pmcp::StdioTransport::new()`.
  - Dispatch split into small helper fns (each under cog 25).

## Verification

- `cargo test -p cargo-pmcp --test agent_dev` — 2 passed (fixed seam + binary,
  in-process sampling-hosted), fully offline.
- `cargo build -p cargo-pmcp` — all three source arms compile (openai-compat
  feature present).
- `cargo test -p cargo-pmcp --test verb_help` — 3 passed (no CLI-surface
  regression from the `DevArgs` change).
- `cargo test -p cargo-pmcp --lib` — 458 passed, 1 ignored (no regression).
- `cargo fmt -p cargo-pmcp --check` — clean.
- `pmat analyze complexity --max-cognitive 25` — no violations on
  `agent/dev.rs` or `agent/run.rs`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Seam exposed as `cargo_pmcp::agent_run`, not
`cargo_pmcp::commands::agent::run`**
- **Found during:** Task 1 (RED compile) / Task 2 wiring.
- **Issue:** The plan's literal call path `cargo_pmcp::commands::agent::run::
  run_fixed_source` is inaccessible from the lib crate: the `commands::*` module
  tree is BIN-ONLY (it cross-depends on `clap`/`GlobalFlags`/CLI siblings and is
  not `pub mod`-exposed in `lib.rs`). An integration test compiles against the
  lib crate, so it cannot reach a bin-only path.
- **Fix:** Mounted `commands/agent/run.rs` into the lib target as `pub mod
  agent_run` via a `#[path]` seam in `lib.rs` — the exact established convention
  used for `templates_agent` and `workbook_explain`. The same file is also
  `pub mod run;` in `commands/agent/mod.rs` for the bin target (dual-mount, like
  `templates/agent.rs`). The test calls `cargo_pmcp::agent_run::run_fixed_source`.
  This satisfies the plan's actual intent (a lib-safe seam reused by the CLI and
  the 110-06 example) and its `must_haves` (run.rs contains `run_fixed_source`,
  dev.rs fixed arm delegates to it).
- **Files modified:** `cargo-pmcp/src/lib.rs`, `cargo-pmcp/tests/agent_dev.rs`.
- **Commit:** `cbff3b5c` (test path also set in `800f0300`).

## Threat Surface

Per the plan's threat model, all three `mitigate` dispositions are honored:
- **T-110-03-01 (Tampering/EoP — endpoint scheme):** `validate_endpoint` runs
  inside `OpenAiCompatSource::with_options` at construction; a remote plain-http
  endpoint returns `CompletionError::Decode`, mapped to an actionable bail naming
  `--allow-insecure-http`.
- **T-110-03-02 (Info disclosure — API key):** the key is sourced from
  `--api-key-env <VAR>` (env-backed, no plaintext CLI arg) and carried as
  `SecretString`; no `println!` ever renders a raw key (the success line prints
  only the source label + outcome tag).
- **T-110-03-03 (DoS — unreachable endpoint):** `.run()` returns a `RunOutcome`;
  a non-`Completed` outcome (including `Failed { error }`, surfaced verbatim as
  it is secret-free) is mapped to an actionable bail naming `--endpoint`/`--source
  fixed`.

No new security surface beyond the plan's threat model.

## Known Stubs

None. `agent dev` is fully wired for all three sources; the 110-01 `bail!("agent
dev: implemented in plan 110-03")` stub is removed.

## Self-Check: PASSED

All created/modified files exist on disk; both task commits (`800f0300`,
`cbff3b5c`) are present in git history.
