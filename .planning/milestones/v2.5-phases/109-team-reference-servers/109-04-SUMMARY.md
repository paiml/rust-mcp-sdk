---
phase: 109-team-reference-servers
plan: 04
subsystem: team-servers
tags: [pmcp-team-servers, approval-mcp, InMemoryTaskStore, ApprovalRepository, webhook, service-owner, atomic-resolve, deterministic-id]

# Dependency graph
requires:
  - phase: 109-team-reference-servers
    plan: 01
    provides: "empty approval/{channels,repository,server} seam, DuplexTransport, feature-gated approval-mcp [[bin]], reqwest behind `webhook`, parking_lot dep, contract D-10/D-11/D-12 rows"
  - phase: 108-pmcp-agent-loop-crate
    provides: "InMemoryTaskStore + .task_store() reuse pattern (adapter/server.rs)"
  - phase: 107-contracts-package-format
    provides: "pmcp-package HumanRole/TeamPackage (the ask family is one tool per human role)"
provides:
  - "ApprovalChannel notify-only trait + ConsoleChannel (default) + feature-gated WebhookChannel (bounded timeout, non-blocking failure, secret non-leak)"
  - "ApprovalRepository (approval-domain state) with atomic first-writer resolve + decision-vs-option-set validation + deterministic id seam (appr-001..)"
  - "build_approval_mcp_server: resolve_approval + get_approval UNNAMESPACED + one team_approval__ask_<role> per human role over InMemoryTaskStore (observable pending->resolved) under a fixed SERVICE_OWNER (D-10)"
  - "approval-mcp HTTP-first dev binary; cfg-safe --webhook-url; env-only webhook secret (V7)"
affects: [109-06-wiring, 109-07-conformance, 109-08-binding-finalize]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Two-store split: InMemoryTaskStore observes the pending->resolved LIFECYCLE; ApprovalRepository is the SOURCE OF TRUTH for domain state + atomic resolution"
    - "Service-owner policy: a fixed SERVICE_OWNER on the task store + a shared repository (Arc) => resolution from ANY client, no auth (D-10)"
    - "Atomic first-writer: whole check-and-set (already-resolved? in option set?) runs under one parking_lot::Mutex; second resolve rejected, never double-applied"
    - "create-record-then-notify: ask mints the pending task + record BEFORE notifying; notify failure is warn-only so the approval is never unreachable"
    - "Notify-only channels: no stdin/TTY/resolution path; webhook secret placed ONLY in the header + redacted Debug, never in a log field (V7)"
    - "cfg-safe CLI flag: --webhook-url always parses; without the `webhook` feature it warns and falls back to console (no compile break)"
    - "Deterministic id seam via object-safe ApprovalIdSource (UuidApprovalIdSource prod, SequentialApprovalIdSource -> appr-001 conformance)"

key-files:
  created: []
  modified:
    - "crates/pmcp-team-servers/src/approval/channels.rs"
    - "crates/pmcp-team-servers/src/approval/repository.rs"
    - "crates/pmcp-team-servers/src/approval/server.rs"
    - "crates/pmcp-team-servers/src/bin/approval_mcp.rs"

decisions:
  - "Double-resolve is REJECTED (ApprovalError::AlreadyResolved carrying the first writer's verdict), not idempotent no-op — the clearer of the two allowed choices; the first verdict is never overwritten"
  - "The approval id (deterministic seam) is distinct from the observable task-store task id (uuid); the record carries task_id so resolve can transition the observable task"
  - "SERVICE_OWNER constant lives in repository.rs and is reused by the server to mint/transition tasks so lifecycle is not client-scoped (D-10)"
  - "Webhook shared secret is read from PMCP_APPROVAL_WEBHOOK_SECRET env only (never a CLI arg) so it never appears in the process table; used only in the x-approval-secret header"
  - "Unknown-role asks are handled by NON-ADVERTISEMENT: only roster roles get an ask tool, so an unknown team_approval__ask_ name yields pmcp's 'tool not found' error, never a panic"

# Metrics
duration: 25min
completed: 2026-07-18
---

# Phase 109 Plan 04: approval-mcp Reference Server (TEAM-03) Summary

**Ships the approval reference server: a notify-only `ApprovalChannel` (`ConsoleChannel` default; feature-gated `WebhookChannel` with a bounded timeout, non-blocking failure, and a header-only/never-logged shared secret), an `ApprovalRepository` that holds the approval-DOMAIN state a `TaskStore` cannot (question, option set, target role, verdict, optional `subject_task_id`/`subject_ref`, D-12) with ATOMIC first-writer `resolve` (second resolve rejected, decision validated against the original option set) behind a `parking_lot::Mutex` and a deterministic `appr-001…` id seam, and `build_approval_mcp_server` advertising `resolve_approval` + `get_approval` UNNAMESPACED plus one `team_approval__ask_<role>` per human role over an `InMemoryTaskStore` (observable pending→resolved lifecycle) under a fixed SERVICE_OWNER so any connected client may resolve (D-10) — with the HTTP-first `approval-mcp` dev binary whose `--webhook-url` is cfg-safe and whose webhook secret is env-only (V7).**

## Performance

- **Duration:** ~25 min
- **Completed:** 2026-07-18
- **Tasks:** 2
- **Files:** 4 modified (`channels.rs`, `repository.rs`, `server.rs`, `bin/approval_mcp.rs` — the 109-01 skeleton stubs this plan resolves)

## Accomplishments

- **`ApprovalChannel` + channels** (`src/approval/channels.rs`): `#[async_trait] ApprovalChannel { notify(&ApprovalAsk) -> Result<(), ChannelError> }`, `ApprovalAsk` (approval id, question, options, target role, optional subject refs), and `ChannelError` (`Timeout`/`Transport`, both non-fatal to the caller). `ConsoleChannel` announces the ask via `tracing::info!` — NO stdin, NO TTY, NO resolution (D-10), always `Ok`. `WebhookChannel` (behind `#[cfg(feature = "webhook")]`) POSTs the ask payload + approval id with an OPTIONAL `x-approval-secret` header; its `reqwest::Client` is built with a bounded connect + request timeout (default 3s, overridable via `with_timeout` for tests) so an offline receiver cannot hang the server task, and every failure path (`status`, `timeout`, transport) is NON-BLOCKING (`tracing::warn!` WITHOUT the secret or URL, then a non-fatal error). The secret value is placed ONLY in the header and the `Debug` impl redacts it (V7 / T-109-04-01).
- **`ApprovalRepository`** (`src/approval/repository.rs`): `ApprovalRecord` (serde camelCase; `id`, `question`, `options`, `targetRole`, `subjectTaskId`, `subjectRef`, `taskId`, `status`, `verdict`), `ApprovalStatus { Pending, Resolved }`, `NewApproval` input, and `ApprovalError` (`NotFound`/`AlreadyResolved`/`InvalidDecision`/`UnknownRole`, thiserror). Backed by `parking_lot::Mutex<HashMap<String, ApprovalRecord>>` + an object-safe `ApprovalIdSource` seam (`UuidApprovalIdSource` prod, `SequentialApprovalIdSource` → `appr-001…` conformance). `resolve` is ATOMIC FIRST-WRITER: the already-resolved check, option-set validation, and set-verdict all run under one mutex acquisition — a second resolve returns `AlreadyResolved` (carrying the first writer's verdict, never overwritten) and an out-of-set decision returns `InvalidDecision` leaving the record `Pending`. `SERVICE_OWNER` constant documents/realizes the D-10 owner policy.
- **`build_approval_mcp_server`** (`src/approval/server.rs`): registers `resolve_approval` + `get_approval` UNNAMESPACED (Pitfall 3 avoided — NOT `team_approval__resolve`) plus exactly one `team_approval__ask_<role>` per `human_roles` entry (`ask_tool_name` lower-cases + slugifies the role label), and wires an `InMemoryTaskStore` via `.task_store(...)`. The `ask` handler mints a pending task (`store.create(SERVICE_OWNER, None)`), creates the repository record FIRST, THEN notifies (a notify failure is warn-only; the ask still returns the approval id → never unreachable). `resolve_approval` calls `repo.resolve` (atomic + option-validated), then transitions the observable task to `Completed` under the same SERVICE_OWNER (a task-store transition failure is logged, not fatal — the record is authoritative), and echoes the stored subject refs + verdict. `get_approval` (read-only hint) echoes the record incl. subject refs (D-12). Unknown roles are simply not advertised, so an unknown `team_approval__ask_` name yields pmcp's "tool not found" error, never a panic.
- **`approval-mcp` binary** (`src/bin/approval_mcp.rs`): thin `#[tokio::main]` + clap `Args` (`--package`, `--data-dir`, `--port`, `--stdio`, `--webhook-url`) mirroring the team-fs/mem-mcp shape. Loads the `TeamPackage`, derives the ask family from `human_roles`, builds a production (uuid-seam) `ApprovalRepository`, and selects `ConsoleChannel` by default or `WebhookChannel` when `--webhook-url` is set. `--webhook-url` is cfg-SAFE: built WITHOUT the `webhook` feature the flag parses but the server warns and falls back to console (no compile break). The optional webhook secret is read from `PMCP_APPROVAL_WEBHOOK_SECRET` env only (never argv). HTTP-first under the `http` feature (`StreamableHttpServer`) with a `--stdio` escape hatch; stdio-only without `http`.

## Task Commits

Each task committed atomically (scoped `git add`, pre-commit quality gate — no `--no-verify`):

1. **Task 1: ApprovalChannel + ConsoleChannel + feature-gated WebhookChannel + webhook mock/timeout tests** — `5034b6bc` (feat)
2. **Task 2: ApprovalRepository + approval-mcp server + HTTP-first binary** — `aa1cc882` (feat)

## Decisions Made

- **Double-resolve REJECTED (not idempotent).** The plan allowed either; rejection via `AlreadyResolved { verdict }` is clearer and provably preserves the first writer's verdict. Tests assert the second resolve errors and the original verdict is intact.
- **Deterministic approval id ≠ observable task id.** The approval id comes from the injectable seam (`appr-001…`); the InMemoryTaskStore mints its own uuid task id. The record carries `task_id` so `resolve_approval` can transition the observable task while conformance keys off the deterministic approval id.
- **Unknown-role safety via non-advertisement.** Rather than a runtime role lookup, only roster roles get an ask tool — an unadvertised `team_approval__ask_<role>` produces pmcp's standard "tool not found" error (proven by test), satisfying the "never panics" contract invariant.
- **Secret hygiene (V7).** The shared secret is env-sourced (never a CLI arg → not in the process table) and used ONLY in the `x-approval-secret` header; `WebhookChannel`'s `Debug` redacts it and no warn/info field ever carries it (grep-verified).

## Deviations from Plan

None — plan executed exactly as written. (Test-helper `Client` bindings were de-`mut`ed after confirming `list_tools`/`call_tool` take `&self`; a cosmetic adjustment, not a behavior change.)

## TDD Gate Compliance

Neither task was tagged `tdd="true"` (both `type="auto"`). Per the repo's mandatory pre-commit `make quality-gate` (which runs `cargo test` and blocks any commit whose tests fail, with no `--no-verify` allowed), implementation and its exhaustive in-file unit + wire tests were committed together per task — matching the sibling 109-02/109-03 execution. All behavioral guarantees (atomic first-writer, decision validation, subject-ref echo, notify-failure resolvability, webhook timeout/secret non-leak, exact tool surface) are covered by committed tests.

## Known Stubs

None for this plan. `src/approval/{channels,repository,server}.rs` and `src/bin/approval_mcp.rs` are now fully implemented (the 109-01 skeleton stubs this plan resolves). Other crate modules (`team/{member,guards,server}`, `compose::wiring`, `conformance::runner`, `bin/team_mcp`) remain documented seams for later 109 plans.

## Threat Flags

None beyond the plan's threat register.
- **T-109-04-01** (secret in logs) mitigated: secret only in the header + redacted `Debug`; no log field carries it (grep-verified — `grep secret channels.rs | grep -iE 'tracing|println|warn!|info!'` → none).
- **T-109-04-02** (webhook egress hang) mitigated: bounded connect+request timeout on the `reqwest::Client`; failure is non-blocking; a dedicated timeout test asserts a non-blocking error within the bounded window against an unresponsive endpoint.
- **T-109-04-03** (double / out-of-set resolution) mitigated: atomic first-writer `resolve` under a `parking_lot::Mutex` (second resolve rejected) + decision-vs-option-set validation, proven by unit + wire tests.
- **T-109-04-04** (webhook SSRF) accepted: opt-in `webhook` feature + `--webhook-url`, notify-only, documented as operator-configured trusted input.
- **T-109-04-05** (unknown-role / unreachable approval) mitigated: unknown roles not advertised (tool-not-found, never panic); create-record-before-notify means a notify failure never yields an unreachable approval (proven by `notify_failure_leaves_approval_resolvable`).
- **T-109-04-SC** (dependency graph) satisfied: no new registry package — `reqwest` already vendored + feature-gated behind `webhook` (off by default), `parking_lot`/`uuid` existing deps.

## Verification Performed

- `cargo test -p pmcp-team-servers --features "approval-mcp" approval` → **17 passed** (repository 6 + server 8 + console 1 + ask_tool_name 1 + wire tests).
- `cargo test -p pmcp-team-servers --features "approval-mcp webhook" approval` → **20 passed** (adds 3 webhook mock-endpoint tests: payload+secret header, no-secret omits header, non-blocking timeout within bound).
- `cargo test -p pmcp-team-servers --features "approval-mcp webhook http"` → **78 passed** (8 suites incl. doctest) — no regression to fs/mem/derive.
- `cargo build -p pmcp-team-servers --features "approval-mcp http" --bin approval-mcp` → exit 0.
- `cargo build -p pmcp-team-servers --features "approval-mcp webhook" --bin approval-mcp` → exit 0.
- `cargo build -p pmcp-team-servers --no-default-features --features approval-mcp --bin approval-mcp` → exit 0 (stdio-only, cfg-safe `--webhook-url`).
- `cargo fmt -p pmcp-team-servers -- --check` → clean; `cargo clippy -p pmcp-team-servers --all-targets --features "approval-mcp webhook http" -- -D warnings` → No issues found.
- Secret non-leak grep + no-stdin grep on `channels.rs` → secret only in `.header(...)` + redacted Debug; `stdin`/`read_line` appear only in doc comments, never as a call.
- Each per-task commit passed the repo pre-commit `make quality-gate` (fmt/clippy/build/test) — commits would have been blocked otherwise.

## Self-Check: PASSED

- Files present: `src/approval/{channels,repository,server}.rs`, `src/bin/approval_mcp.rs` — all on disk with implemented bodies.
- Commits present in git history: `5034b6bc` (Task 1), `aa1cc882` (Task 2).

## Next Phase Readiness

- 109-06 wiring and 109-07 conformance can drive approval-mcp over `DuplexTransport` (exact `resolve_approval` + `get_approval` + N `team_approval__ask_<role>` surface asserted here) with `ApprovalRepository::deterministic()` for reproducible `appr-001…` fixtures. 109-08 flips the `binding.yaml` `approval_tool_surface` entry to `status: implemented`.
- No blockers.

---
*Phase: 109-team-reference-servers*
*Completed: 2026-07-18*
