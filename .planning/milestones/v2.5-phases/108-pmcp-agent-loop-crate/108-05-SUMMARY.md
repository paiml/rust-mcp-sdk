---
phase: 108-pmcp-agent-loop-crate
plan: 05
subsystem: api
tags: [tool-invoker, tasks-augmented, connector-factory, slot-resolver, resolve-agent, secrets]

# Dependency graph
requires:
  - phase: 108-pmcp-agent-loop-crate (plan 02)
    provides: ToolInvoker/ToolCall/ToolCallResult seams + ResolvedAgentConfig contract
  - phase: 108-pmcp-agent-loop-crate (plan 01)
    provides: pmcp 2.17.0 (wait_for_related_task, WaitForTaskOptions, task-augmented CallToolResult)
  - phase: 107 (pmcp-package)
    provides: AgentPackage, ConfigSlot/SlotType, detect_deviation, ComponentRef
provides:
  - "ClientToolInvoker — tasks-aware ToolInvoker over a ConnectorClient: drives task-augmented results via wait_for_related_task under a host-configured hard max_poll_duration_secs cap (AGNT-08)"
  - "invoke_batch override: bounded-concurrency buffered(N) dispatch, input-order + id-matched (D-07)"
  - "ConnectorClient/ConnectorClientFactory object-safe seams (unconditional/wasm-clean) + UrlConnectorClientFactory over StreamableHttpTransport behind the url-connector feature (BLOCKER-2)"
  - "SlotResolver seam (+resolve_endpoint) with EnvVarResolver + ProgrammaticBuilder impls; deviation warns-and-runs (D-15)"
  - "RedactedSecret redaction wrapper (Debug/Display never leak, ASVS V7)"
  - "build_endpoint_map (connector-name -> endpoint, D-16) + resolve_agent (AgentPackage -> ResolvedAgentConfig, AGNT-09)"
affects: [plan-108-06-adapter, AGNT-08, AGNT-09]

# Tech tracking
tech-stack:
  added:
    - "crates/pmcp-agent: url 2.5 (optional, url-connector-gated) — construct StreamableHttpTransportConfig.url"
    - "crates/pmcp-agent dev-dep: semver 1 — construct AgentPackage in config_resolver tests"
  patterns:
    - "Object-safe transport erasure: ConnectorClient trait fronts a transport-generic pmcp::Client<T> (which is NOT object-safe) so the invoker holds Arc<dyn ConnectorClient>"
    - "Hard-cap-then-delegate: invoker sets WaitForTaskOptions{ max_poll_duration_secs: Some(cap) }.or_from_metadata(&meta) and reuses the SDK poll primitive — no hand-rolled poll loop"
    - "buffered(N) for bounded parallel dispatch: caps concurrency AND yields in input order, so ids stay correlated without an index sort"
    - "Redaction-by-type: RedactedSecret's Debug prints RedactedSecret(***); the value is reachable only via expose()"
    - "warn-and-run deviation: construct a proposed SlotType from the resolved value, let detect_deviation gate the tracing::warn!, never error (D-15)"

key-files:
  created:
    - crates/pmcp-agent/src/invoker/client.rs
    - crates/pmcp-agent/src/invoker/factory.rs
    - crates/pmcp-agent/src/config/resolver.rs
    - crates/pmcp-agent/src/config/endpoint.rs
    - crates/pmcp-agent/tests/invoker_task_augmented.rs
    - crates/pmcp-agent/tests/config_resolver.rs
  modified:
    - crates/pmcp-agent/src/invoker/mod.rs
    - crates/pmcp-agent/src/config/mod.rs
    - crates/pmcp-agent/src/lib.rs
    - crates/pmcp-agent/Cargo.toml

requirements: [AGNT-08, AGNT-09]

# Verification
verification:
  - "cargo test -p pmcp-agent --test invoker_task_augmented --test config_resolver -- --test-threads=1: 5 + 8 tests pass"
  - "cargo build -p pmcp-agent (default) compiles the invoker with NO active StreamableHttpTransport reference (all such code is behind #[cfg(feature=\"url-connector\")])"
  - "cargo build -p pmcp-agent --features url-connector compiles the URL impl on native"
  - "cargo clippy -p pmcp-agent --all-targets AND --features url-connector: clean; cargo fmt -p pmcp-agent --check: clean"
---

# Plan 108-05 Summary: tasks-aware tool invoker + connector factory + package-config resolver

Shipped the three Wave-3 seams that turn a resolved package into runnable effects:
the tasks-aware `ClientToolInvoker` (AGNT-08), the object-safe `ConnectorClientFactory`
(URL transport impl), and the `SlotResolver` + `resolve_agent` package-config
composition path (AGNT-09).

## Tasks

1. **ClientToolInvoker + ConnectorClientFactory (AGNT-08)** — `ConnectorClient`/
   `ConnectorClientFactory` are unconditional object-safe traits (they compile on
   wasm32); the concrete `UrlConnectorClientFactory` (wraps
   `pmcp::Client<StreamableHttpTransport>`, validates the `http(s)` scheme) sits behind
   the `url-connector` feature so the default/wasm build never references the native
   transport (BLOCKER-2). `ClientToolInvoker::invoke` calls `call_tool`, and when the
   result carries `related_task()` drives it to terminal via
   `wait_for_related_task(&meta, WaitForTaskOptions{ max_poll_duration_secs: Some(cap), .. }.or_from_metadata(&meta))`
   — the host cap is authoritative, so it can never poll forever. `invoke_batch`
   overrides the sequential seam default with `buffered(N)` bounded-concurrency dispatch
   that preserves input order and ids. Errors (incl. the never-completing-task timeout)
   surface as `ToolCallResult` error data, not panics.
2. **SlotResolver + resolve_agent + endpoint map (AGNT-09)** — `SlotResolver` (with
   `resolve_slot` + `resolve_endpoint`) has two impls: `EnvVarResolver` (conventional
   env-var names, `-`→`_`, uppercased, optional prefix) and `ProgrammaticBuilder`
   (explicit in-memory values/secrets/endpoints). Behavior-relevant slots resolve to a
   plain value (env/explicit override, else the tested default), and a deviation from the
   package's `tested_value` triggers a loud `tracing::warn!` via `detect_deviation` then
   PROCEEDS (D-15). Identity-bearing secret slots resolve into a `RedactedSecret` whose
   `Debug`/`Display` reveal nothing (ASVS V7); a missing required secret is a typed
   `ResolveError::MissingSlot`. `build_endpoint_map` assembles the connector-name →
   endpoint map (D-16), and `resolve_agent` composes instructions, flattened tool
   selection, clamped integer limits, I/O schemas, resolved model, and the endpoint map
   into a full `ResolvedAgentConfig`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `url` as an optional (`url-connector`-gated) dependency**
- **Found during:** Task 1
- **Issue:** `StreamableHttpTransportConfig.url` is a `url::Url`; the concrete URL
  connector cannot construct it without naming the `url` crate, which was not a direct
  dependency of `pmcp-agent`.
- **Fix:** Added `url = { version = "2.5", optional = true }` and put it in the
  `url-connector = [..., "dep:url"]` feature so the default/wasm build is unaffected.
  `url` is already resolved in the workspace tree via `pmcp` (not a new/untrusted
  install), so this is not a package-legitimacy checkpoint.
- **Files modified:** crates/pmcp-agent/Cargo.toml
- **Commit:** 4ecf6ddf

**2. [Rule 3 - Blocking] Added `semver` as a dev-dependency**
- **Found during:** Task 2
- **Issue:** The end-to-end `resolve_agent` test constructs an `AgentPackage`, whose
  `version` (`semver::Version`) and connector `range` (`semver::VersionReq`) fields
  require naming the `semver` crate, which was not in `pmcp-agent`'s dev-deps.
- **Fix:** Added `semver = "1"` under `[dev-dependencies]` (test-only; never affects the
  published build). Already in the workspace tree via `pmcp-package`.
- **Files modified:** crates/pmcp-agent/Cargo.toml
- **Commit:** f9460d82

## Design Notes

- **Test double for the invoker:** the task-augmented tests drive `ClientToolInvoker`
  through a controllable mock `ConnectorClient` (the crate's actual collaboration seam)
  rather than a full duplex server+task-store. The mock builds GENUINE `CallToolResult`s
  via `CallToolResult::with_related_task`, so the invoker exercises the real
  `related_task()` accessor and real `WaitForTaskOptions` plumbing; the mock asserts the
  invoker passed a hard cap and simulates a never-completing task by returning
  `Error::timeout` under that cap. The real SDK poll-budget loop is already tested in
  `pmcp`; this crate's responsibility (set the cap, map outcomes to data) is what the
  tests pin.
- **Command/stdio transport is a documented follow-up** (rustdoc in `factory.rs`): URL
  endpoints cover the AGNT-05/AGNT-09 targets and a command transport adds no new loop
  behavior — only a second `ConnectorClient` impl behind the same seam.

## TDD Gate Compliance

The plan marks both tasks `tdd="true"`. This crate's zero-defect policy (CLAUDE.md) means
every commit must build+clippy+test clean, so a separate RED commit of a
non-compiling/failing test is not committable here. Each task was developed test-first
locally (mock/test written, run to observe failure, then implemented to green) and
committed as a single cohesive `feat` unit once green. Git log therefore shows
`feat(108-05)` commits rather than a `test`→`feat` pair; the behavior-driving tests
(`invoker_task_augmented.rs`, `config_resolver.rs`) are present and green in the same
commit as the implementation.

## Threat Model Coverage

- T-108-05-01 (secret disclosure): `RedactedSecret` — Debug asserts `RedactedSecret(***)`,
  secret-absence test passes.
- T-108-05-02 (silent config drift): `detect_deviation` + `tracing::warn!` (D-15);
  deviation test asserts Ok (warn-and-run).
- T-108-05-03 (unbounded polling): host-configured `max_poll_duration_secs` cap;
  never-completing-task test returns a timeout error, not a hang.
- T-108-05-04 (unbounded batch concurrency): `buffered(N)`; bounded-concurrency test
  asserts peak in-flight == the bound.
- T-108-05-05 (key sent to wrong endpoint): `UrlConnectorClientFactory` validates the
  `http(s)` scheme and rejects others with `UnsupportedScheme`.

## Self-Check: PASSED
