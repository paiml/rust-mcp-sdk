---
phase: 109-team-reference-servers
plan: 06
subsystem: team-servers
tags: [pmcp-team-servers, compose, wiring, TeamRuntime, in-process, duplex-transport, cfg-gating, fail-closed, transactional-startup]

# Dependency graph
requires:
  - phase: 109-team-reference-servers
    plan: 01
    provides: "derive_attachment/AttachmentSet, PackageResolver seam, DuplexTransport, compose::wiring documented stub"
  - phase: 109-team-reference-servers
    plan: 05
    provides: "MemberHandle::spawn, resolve_member_factory, build_team_mcp_server, LocalDirPackageResolver, MemberId/MemberTaskForwarding, member-llm feature"
  - phase: 109-team-reference-servers
    plan: 02
    provides: "build_team_fs_server + LocalDirBackend"
  - phase: 109-team-reference-servers
    plan: 03
    provides: "build_mem_mcp_server + InMemoryMemoryBackend"
  - phase: 109-team-reference-servers
    plan: 04
    provides: "build_approval_mcp_server + ApprovalRepository + ConsoleChannel/ApprovalChannel"
  - phase: 108-pmcp-agent-loop-crate
    provides: "AgentServer::builder(pkg, config, factory, invoker, store), resolve_agent, InMemoryStore, FixedSourceFactory, ProgrammaticBuilder/SlotResolver"
provides:
  - "TeamRuntimeBuilder: collects every runtime seam (PackageResolver, SlotResolver, completion override, invoker, store factory, forwarding, data root, approval channel, EnabledServers policy) with documented defaults"
  - "TeamRuntime + TeamRuntime::start: derive-driven in-process composition of team-mcp/approval-mcp/team-fs/mem-mcp + members over in-memory DuplexTransport pairs (D-01/D-04)"
  - "Per-branch + per-field cfg-gating so --no-default-features --features team-fs compiles the runtime skeleton; requested-but-uncompiled/unknown/policy-disabled server FAILS CLOSED (RuntimeError::UnsupportedServer)"
  - "Transactional startup (aborts already-spawned hosting tasks on later failure) + explicit shutdown (returns joined count) + Drop safety net"
  - "EnabledServers opt-in policy type; hosted_task_count() teardown observability"
affects: [109-07-conformance, 110-cargo-pmcp-team-dev]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Member wiring == the team-mcp subsystem: PackageResolver/SlotResolver/completion-override/invoker/store/forwarding seams are all cfg-gated behind `team-mcp` (their consumer), so reduced-feature builds stay dead-code-clean while feature-typed fields (approval channel, forwarding) satisfy the per-field cfg requirement"
    - "Fail-closed opt-in: match on ComponentRef.name() → EnabledServers policy check → per-feature cfg branch → UnsupportedServer for unknown/disabled/uncompiled (never silently ignored)"
    - "Transactional startup via a Result<TeamRuntime, (RuntimeError, Vec<JoinHandle>)> wiring core: the error arm hands the accumulated hosting-task list back to build() for a bulk abort; member handles built-so-far self-terminate on drop (their in-memory transports close)"
    - "In-memory host() helper spawns the server task BEFORE client.initialize so a failed handshake still aborts transactionally"
    - "Shared member path: resolve_agent + resolve_member_factory + MemberHandle::spawn — the exact helpers the team-mcp dev binary uses (no inline SlotResolver/factory re-implementation)"

key-files:
  created:
    - "crates/pmcp-team-servers/tests/small_team.rs"
  modified:
    - "crates/pmcp-team-servers/src/compose/wiring.rs"
    - "crates/pmcp-team-servers/src/compose/mod.rs"

decisions:
  - "Member-wiring seams gated behind `team-mcp` (not just the two feature-typed fields) because member AgentServer construction IS the team-mcp subsystem — keeps every reduced-feature combo (team-fs/mem-mcp/approval-mcp only) warning-clean without stray #[allow(dead_code)]"
  - "Team-of-one keeps the sole MemberHandle in the runtime (exposed via solo_member(), proven live via dispatch) rather than building a needless team-mcp Server"
  - "shutdown() returns the joined hosting-task count so teardown is observable; a surviving in-memory client keeps the server actor alive (Server::run detaches its actor and only ends on transport EOF), so post-shutdown reachability is NOT a valid leak probe — the joined-count + prompt-completion assertion is"
  - "EnabledServers policy provides a deterministic, all-features fail-closed test (disable a compiled server) that mirrors the reduced-feature uncompiled path"

requirements-completed: [TEAM-01]

# Metrics
duration: 40min
completed: 2026-07-18
---

# Phase 109 Plan 06: In-Process Small-Team Wiring API (TEAM-01) Summary

**Implements D-01: a `TeamRuntimeBuilder` + `TeamRuntime` that compose a whole small team — the derive-selected servers plus every member `AgentServer` — into ONE tokio process over in-memory `DuplexTransport` pairs (D-04, no sockets). The builder collects every seam the 109-06 review found missing (a `PackageResolver` for ComponentRef→AgentPackage, a `SlotResolver`, an explicit completion-source override, the `AgentServer` config/invoker/store seams, a data root, an approval channel, and an `EnabledServers` opt-in policy) with documented defaults. `TeamRuntime::start` runs `derive_attachment`, resolves each member `ComponentRef` into an `AgentPackage` via the injected resolver, builds each member `AgentServer` with the runtime seams, and wires team-mcp (iff members≥2), approval-mcp (iff human_roles non-empty), and the opt-in team-fs/mem-mcp extras — each branch cfg-gated on its server feature so `--no-default-features --features team-fs` still compiles the skeleton, and a requested-but-uncompiled / unknown / policy-disabled server FAILS CLOSED with `RuntimeError::UnsupportedServer`. Member LLM comes from the SHARED `resolve_member_factory` (the same helper the team-mcp dev binary uses). Startup is transactional (a later failure aborts every already-spawned hosting task); shutdown/Drop are explicit. "Small team, one process", the team-of-one degenerate case, fail-closed opt-ins, and clean shutdown are all proven on an injected `FixedSource` override.**

## Performance

- **Duration:** ~40 min
- **Completed:** 2026-07-18
- **Tasks:** 2
- **Files:** 3 changed (1 created: `tests/small_team.rs`; 2 modified: `src/compose/wiring.rs`, `src/compose/mod.rs`)

## Accomplishments

- **`TeamRuntimeBuilder`** (`src/compose/wiring.rs`): collects `resolver: Arc<dyn PackageResolver>`, `slot_resolver: Arc<dyn SlotResolver>`, `completion_override: Option<Arc<dyn CompletionSourceFactory>>` (the explicit DI seam — tests pass `FixedSource`, production `None`), `invoker: Arc<dyn ToolInvoker>` (default no-op), a per-member `store_factory` (default `InMemoryStore`), `forwarding: MemberTaskForwarding`, `data_root: PathBuf`, `approval_channel: Arc<dyn ApprovalChannel>` (default `ConsoleChannel`), and an `EnabledServers` policy. Every server-specific field whose TYPE lives behind a feature (`approval_channel`, `forwarding`) is per-field `#[cfg]`-gated; the member-wiring seams are gated behind `team-mcp` (their sole consumer), so `--no-default-features --features team-fs` compiles the struct with zero dead-code warnings. Setters mirror the gating; `#[must_use]` throughout.
- **`TeamRuntime::start` / `TeamRuntimeBuilder::build`**: (1) `derive_attachment(pkg)`; (2) for each member `resolver.resolve_agent` → `AgentPackage`, `resolve_agent` (shared config) + `resolve_member_factory` (shared LLM factory) → `AgentServer::builder(pkg, config, factory, invoker, store).build()` → `MemberHandle::spawn` over a `DuplexTransport` pair; (3) `#[cfg(team-mcp)]` if `attachment.team_mcp` build+host team-mcp, else keep the sole member; (4) `#[cfg(approval-mcp)]` if `attachment.approval_mcp` build+host approval-mcp from `human_roles`; (5) opt-in loop hosts team-fs (`LocalDirBackend` at `data_root`) / mem-mcp, **failing closed** for unknown/disabled/uncompiled names. Everything is hosted over in-memory `DuplexTransport` via an initialized `pmcp::Client` — grep confirms NO `TcpListener`/socket bind.
- **Transactional startup**: the wiring core returns `Result<TeamRuntime, (RuntimeError, Vec<JoinHandle>)>`; on any error `build()` aborts every hosting task spawned so far. `host()` tracks the serving task BEFORE `client.initialize`, so even a failed handshake is covered. Member handles built before a failure are dropped, closing their transports so the member tasks self-terminate.
- **Explicit lifecycle**: `shutdown(self) -> usize` aborts + joins every tracked hosting task (returning the joined count for leak assertions) and drops the clients/sole-member (closing transports so the servers' inner actor tasks reach EOF). A `Drop` safety net aborts any still-tracked task if `shutdown` was skipped. Accessors: `attachment()`, `team_mcp_client()`, `approval_client()`, `team_fs_client()`, `mem_client()`, `solo_member()`, `hosted_task_count()`.
- **`RuntimeError`** (thiserror): `Resolve`, `UnsupportedServer { name }`, `Build`, `Spawn`. **`EnabledServers`** opt-in policy (`all`/`none`/`with`/`without`/`permits`, default all). Both re-exported from `compose::mod`.
- **`tests/small_team.rs`** (5 tests, all on an injected `FixedSource` — CI-deterministic, no live LLM/network): (1) a 2-member + 1-human + `[team-fs, mem-mcp]` team brings up team-mcp (2 `team_mcp__*` tools), approval-mcp (`resolve_approval`/`get_approval`/`team_approval__ask_*`), team-fs (11 `fs__*`), and mem-mcp (6 `mem__*`) in one process, each driven through its client; (2) a team-of-one/zero-human wires ONLY the sole member (asserted live via `dispatch`), no other servers; (3) an unknown opt-in and (4) a policy-disabled compiled opt-in both fail closed with `UnsupportedServer`; (5) shutdown aborts+joins every hosting task promptly (no leak).

## Task Commits

Each task committed atomically (scoped `git add`, pre-commit `make quality-gate` passed — no `--no-verify`):

1. **Task 1: TeamRuntimeBuilder + TeamRuntime (derive-driven, all seams, cfg-gated branches, fail-closed, transactional startup)** — `a1c37ed7` (feat)
2. **Task 2: small-team-one-process + team-of-one + fail-closed + shutdown integration tests (+ observable teardown API)** — `19c0b6f5` (test)

## Decisions Made

- **Member-wiring seams gated behind `team-mcp`.** The plan requires per-field `#[cfg]` only for fields whose TYPE lives behind a feature (`approval_channel`, `forwarding`). But the resolver/slot-resolver/override/invoker/store seams are consumed ONLY by member wiring (the team-mcp subsystem), so gating them behind `team-mcp` too keeps every reduced-feature combo warning-clean without any `#[allow(dead_code)]`. Verified clippy-clean under `team-fs`, `mem-mcp`, `approval-mcp`, `team-mcp`, default, and all-features.
- **Team-of-one keeps the sole `MemberHandle`** (exposed via `solo_member()`, proven live via `dispatch`) rather than standing up a pointless single-member team-mcp Server — matching `derive_attachment`'s "team-mcp iff ≥2 members".
- **`shutdown()` returns a joined count for observable teardown.** `Server::run` detaches its transport-actor task, which only ends on transport EOF; a surviving in-memory client keeps the actor alive, so "call fails after shutdown" is NOT a valid leak probe when a client clone is held. The valid assertion is: every tracked hosting task is aborted+joined and shutdown completes promptly.
- **`EnabledServers` gives a deterministic all-features fail-closed test.** Disabling a compiled server via the policy exercises the same `UnsupportedServer` path a reduced-feature build takes for an uncompiled server, so the fail-closed contract is proven without needing a separate reduced-feature test binary (the reduced-feature COMPILE is verified by the build command).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Added observable teardown API (`hosted_task_count` + `shutdown` returns joined count)**
- **Found during:** Task 2
- **Issue:** The plan asks the shutdown test to assert "no spawned task leaks". `Server::run` detaches its inner transport-actor task, and an in-memory server stays alive as long as ANY client holds the transport — so the intuitive "the server is unreachable after shutdown" assertion is unobservable (a held client keeps it up) and would false-fail.
- **Fix:** Made `shutdown` return the number of hosting tasks it aborted+joined and added `hosted_task_count()`; the test asserts every tracked task is torn down and shutdown completes within a timeout (a hung/leaked task would block).
- **Files modified:** `src/compose/wiring.rs` (committed in `19c0b6f5`)

**Total deviations:** 1 auto-fixed (Rule 2). Additive, preserves the plan's intent and the crate's threat/dependency posture (no new deps).

## Threat Model Compliance

- **T-109-06-01 (member LLM key handling):** keys flow through the SHARED `resolve_member_factory` / `SlotResolver` (`RedactedSecret`) path from 109-05; never logged. All tests use an injected `FixedSource` override, so no real secret is ever resolved.
- **T-109-06-02 (spawned member tasks leak / partial-startup failure):** startup is transactional — a later failure aborts every already-spawned hosting task; member handles built-so-far drop and self-terminate. Explicit `shutdown` (aborts+joins, returns count) + `Drop` safety net; the leak-free teardown is tested.
- **T-109-06-03 (requested-but-uncompiled/unknown opt-in silently ignored):** fail closed with `RuntimeError::UnsupportedServer` across unknown names, policy-disabled servers, and uncompiled features (cfg `not(feature)` arms); proven by the unknown-opt-in and policy-disabled tests.
- **T-109-06-SC (dependency graph):** no new registry package — the runtime composes only in-repo crates (`pmcp`, `pmcp-agent`, `pmcp-package`) and existing deps (`tokio`, `thiserror`). `grep` shows no `TcpListener`/socket bind in `wiring.rs`.

## Known Stubs

None for this plan. `src/compose/wiring.rs` is now fully implemented (the 109-01 skeleton stub this plan resolves). The only remaining crate seam is `conformance::runner` (109-07).

## Threat Flags

None — the runtime introduces no new network endpoint or trust-boundary surface: all wiring is in-process over in-memory `DuplexTransport` pairs (no sockets). It composes the already-registered per-server surfaces from 109-02..05.

## Verification Performed

- `cargo test -p pmcp-team-servers --test small_team --all-features` → **5 passed** (small-team-one-process, team-of-one, unknown-opt-in fail-closed, policy-disabled fail-closed, shutdown no-leak).
- `cargo test -p pmcp-team-servers --test small_team` (default features) → **5 passed**.
- `cargo test -p pmcp-team-servers --all-features` → **111 passed** (10 suites incl. doctest) — no regression to fs/mem/approval/team/derive.
- `cargo build -p pmcp-team-servers --all-features` → exit 0; `cargo build -p pmcp-team-servers --no-default-features --features team-fs` → exit 0 (selective-feature compilation).
- `cargo clippy -p pmcp-team-servers --all-features -- -D warnings` and per-single-feature (`team-fs`/`mem-mcp`/`approval-mcp`/`team-mcp`) + default + `--tests --all-features` → all "No issues found".
- `cargo fmt -p pmcp-team-servers -- --check` → clean.
- Fail-closed + no-socket confirmed: `wiring.rs` matches on `ComponentRef.name()` → `EnabledServers` policy → per-feature `#[cfg]` → `UnsupportedServer`; no `TcpListener`/bind present.
- Each per-task commit passed the repo pre-commit `make quality-gate` (fmt/clippy/build/test) — commits would have been blocked otherwise.

## Self-Check: PASSED

- Files present: `src/compose/wiring.rs`, `src/compose/mod.rs`, `tests/small_team.rs` — all on disk with implemented bodies.
- Commits present in git history: `a1c37ed7` (Task 1), `19c0b6f5` (Task 2).

## Next Phase Readiness

- 109-07 fills the last seam (`conformance::runner`); it can drive the composed servers over `DuplexTransport` exactly as `TeamRuntime` does.
- Phase 110 `cargo pmcp team dev` becomes a thin wrapper over `TeamRuntimeBuilder` — one definition format (TeamPackage) from laptop to platform, sharing member-LLM + package resolution with the reference dev binary.
- No blockers.

---
*Phase: 109-team-reference-servers*
*Completed: 2026-07-18*
