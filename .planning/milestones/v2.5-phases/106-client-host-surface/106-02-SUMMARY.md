---
phase: 106-client-host-surface
plan: 02
subsystem: api
tags: [mcp, sampling, approval, human-in-the-loop, capabilities, fuzz, denial-of-wallet, wasm]

# Dependency graph
requires:
  - phase: 106-client-host-surface (plan 01)
    provides: "pmcp::client::host module (HostSamplingHandler, ApprovalDecision, PreflightApproval/SamplingResultReview types, ClientHostRegistry, dispatch_host_request + classify_host_request), tests/common/duplex.rs harness"
provides:
  - "Two-stage sampling approval INVOCATION: mandatory preflight gate (runs before the handler — a Deny prevents the LLM call, no tokens billed) + optional post-generation result-review (default pass-through)"
  - "Sanitized -32603 'request denied by host policy' on denial; raw deny reason logged locally via tracing::warn!, never forwarded; connection kept alive (duplex-proven)"
  - "HOST-05 registry-authoritative capability derivation in Client::initialize: handler-absent => field forced None (anti-capability-lie); handler-present => caller sub-capability detail preserved, default() only when caller set none; tasks/experimental untouched"
  - "fuzz/fuzz_targets/client_host_routing.rs driving the REAL routing path (parse_request -> classify_host_request) + the param serde boundary"
  - "pmcp 2.16.0 (additive host surface) + cargo-pmcp 0.17.4 scaffold PMCP_VERSION pin in lockstep"
affects: [108-sampling-source, 111-sampling-hosting-docs, pmcp-agent, pmcp.run-durable-host]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Two-stage host access control: mandatory preflight (pre-LLM, owned params) + optional result-review (post-LLM, owned params + result), both boxed futures, sampling-path only"
    - "Registry-authoritative capability derivation (handler presence is the single source of truth; caller detail preserved, never independently assertable)"
    - "Fuzz the real dispatch classification (bytes -> JSONRPCRequest -> parse_request -> classify_host_request), not standalone serde"
    - "Empty [workspace] in cargo-fuzz crate so it resolves as its own root under git-worktree checkouts"

key-files:
  created:
    - tests/client_host_approval.rs
    - fuzz/fuzz_targets/client_host_routing.rs
  modified:
    - src/client/mod.rs
    - src/client/host/mod.rs
    - fuzz/Cargo.toml
    - Cargo.toml
    - cargo-pmcp/src/templates/workbook_server.rs
    - cargo-pmcp/Cargo.toml
    - tests/client_host_roundtrip.rs

key-decisions:
  - "Preflight is the mandatory pre-handler gate (owned CreateMessageParams cloned once up front): a Deny returns -32603 WITHOUT invoking the handler, genuinely closing the denial-of-wallet hole (T-106-05). Default (no callback) = allow"
  - "Result-review is post-generation and optional; default (no callback) is pass-through. It sees the produced completion and can still Deny"
  - "Policy denial is a distinct helper (host_policy_denied) returning generic -32603 'request denied by host policy'; the callback's Deny(reason) is tracing::warn!-logged locally, never forwarded (T-106-11)"
  - "HOST-05 rule is per-field and exact: absent=>None (locked anti-lie), present+caller-None=>default(), present+caller-detail=>preserve; tasks/experimental never touched; no independent public setter for the three host fields"
  - "classify_host_request + HostRequestKind exposed as #[doc(hidden)] pub (not stable API) so the routing fuzz target can drive the real classifier"

patterns-established:
  - "Pattern: dispatch_host_sampling flows preflight -> handler -> result-review with a single owned-params clone, staying well under cognitive-complexity 25"
  - "Pattern: derive_host_capabilities mutates a &mut ClientCapabilities in place, called from initialize before store+send"

requirements-completed: [HOST-04, HOST-05]

# Metrics
duration: 37min
completed: 2026-07-17
---

# Phase 106 Plan 02: Client Host Approval + Capability Derivation Summary

**Two-stage sampling approval (mandatory preflight gate that prevents the LLM call on Deny + optional post-generation result-review), registry-authoritative host capability derivation (anti-capability-lie with caller-detail preservation), a real parse_request->classify_host_request fuzz target, and the pmcp 2.16.0 minor bump with its cargo-pmcp scaffold-pin tripwire.**

## Performance

- **Duration:** ~37 min
- **Started:** 2026-07-17T22:33Z
- **Completed:** 2026-07-17T23:10Z
- **Tasks:** 4 (+1 quality-gate follow-up)
- **Files created:** 2  **Files modified:** 7

## Accomplishments
- **HOST-04 (real access-control gate):** `dispatch_host_sampling` now runs a **mandatory preflight approval BEFORE the handler** — an `ApprovalDecision::Deny` returns a sanitized `-32603` and the sampling handler is never invoked (no LLM call, no tokens billed), the genuine denial-of-wallet fix. An **optional result-review** stage sees the produced completion and can Deny post-generation; its default is pass-through. Denials are generic `"request denied by host policy"`; the raw reason is `tracing::warn!`-logged locally and never forwarded. Applied to the sampling path only (not elicitation/roots).
- **HOST-05 (truthful advertisement):** `Client::initialize` derives the three host capability fields from the registry with the exact locked rule — handler absent ⇒ field forced `None` (anti-capability-lie), handler present + caller `None` ⇒ `default()`, handler present + caller-configured detail ⇒ **preserved**. `tasks`/`experimental` and all other fields untouched; no independent public setter.
- **Honest fuzz coverage:** `fuzz/fuzz_targets/client_host_routing.rs` drives the real routing path `bytes -> JSONRPCRequest<Value> -> parse_request -> classify_host_request` (plus the `CreateMessageParams`/`ElicitRequestParams` serde boundary). `classify_host_request` + `HostRequestKind` exposed as `#[doc(hidden)] pub`. Smoke run `cargo fuzz run client_host_routing -- -runs=2000` completed clean (exit 0, cov 249, no panics).
- **Release-ready bump:** pmcp `2.15.0 -> 2.16.0`, the `cargo-pmcp` `workbook_server.rs` `PMCP_VERSION` pin bumped in lockstep (the exact drift-guard tripwire that broke the 2.15.0 release), and `cargo-pmcp` `0.17.3 -> 0.17.4` so the scaffold-pin change ships.
- **Duplex proof (incl. orchestrator addendum):** `tests/client_host_approval.rs` proves (1) a preflight-denied sampling request returns `-32603`, the handler is never invoked, and a subsequent client request completes (connection survived); and (2) a KNOWN host method (`elicitation/create`) with no registered handler returns `-32601` and a subsequent request completes.

## Task Commits

Each task was committed atomically:

1. **Task 1: Preflight approval gate + result-review + duplex denial test** - `fc9873a3` (feat)
2. **Task 2: Capability derivation — registry-authoritative anti-lie w/ preservation** - `69ee6381` (feat)
3. **Task 3: Routing fuzz target (parse_request -> classify_host_request)** - `62b0ffbb` (test)
4. **Task 4: Version bump pmcp 2.16.0 + cargo-pmcp scaffold pin** - `ffd50998` (chore)

**Quality-gate follow-up:** `ed5fdaa0` (style — fmt + clippy pedantic on the new tests)

## Files Created/Modified
- `src/client/mod.rs` - Two-stage `dispatch_host_sampling` (preflight/handler/result-review), `host_policy_denied` helper, `derive_host_capabilities` + call in `initialize`; unit tests for both features.
- `src/client/host/mod.rs` - `classify_host_request` + `HostRequestKind` promoted to `#[doc(hidden)] pub` for the fuzz crate.
- `tests/client_host_approval.rs` - Duplex preflight-deny (handler-not-invoked + connection survives) + known-unhandled `-32601` connection-survival (addendum).
- `fuzz/fuzz_targets/client_host_routing.rs` - Routing + param-serde fuzz target.
- `fuzz/Cargo.toml` - `[[bin]] client_host_routing`; empty `[workspace]` (worktree resolution).
- `Cargo.toml` - pmcp `2.16.0`.
- `cargo-pmcp/src/templates/workbook_server.rs` - `PMCP_VERSION = "2.16.0"`.
- `cargo-pmcp/Cargo.toml` - version `0.17.4`.
- `tests/client_host_roundtrip.rs` - doc backtick fix (clippy::doc_markdown) so the shared gate is green.

## Decisions Made
- **Preflight is the gate, result-review is the audit:** owned params are cloned once; preflight runs before the handler (Deny = no LLM call), result-review runs after (Deny = suppress the produced completion). Both default to allow/pass-through.
- **Sanitized denials:** a dedicated `host_policy_denied` returns the generic `-32603` message; raw `Deny(reason)` is only ever logged locally (`tracing::warn!`), never sent to the server.
- **Registry is the single source of truth for host capabilities:** derivation is a pure function of which handlers are registered; caller detail is preserved only when a handler exists.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] cargo-fuzz crate could not resolve its workspace under a git-worktree checkout**
- **Found during:** Task 3 (routing fuzz target)
- **Issue:** `cd fuzz && cargo build --bin client_host_routing` (the plan's verify command) failed with *"current package believes it's in a workspace when it's not"* — cargo walked past the worktree root (which `workspace.exclude`s `fuzz`) up to the main-repo root workspace. The pre-existing `fuzz_peer_handle` target failed identically, confirming an environmental (worktree) limitation, not a fault in the new target.
- **Fix:** Added an empty `[workspace]` table to `fuzz/Cargo.toml` (the canonical cargo-fuzz pattern) so the fuzz crate resolves as its own workspace root. Harmless in the main repo (fuzz is already `workspace.exclude`d there).
- **Files modified:** fuzz/Cargo.toml
- **Verification:** `cargo build --bin client_host_routing` builds clean; `cargo fuzz run client_host_routing -- -runs=2000` completed (exit 0, no panics).
- **Committed in:** 62b0ffbb (Task 3 commit)

**2. [Rule 3 - Blocking] Pre-existing clippy::doc_markdown in plan-01 test blocked the shared quality gate**
- **Found during:** Phase-end `make quality-gate`
- **Issue:** The current toolchain (clippy 1.97) flags `tool_choice`/`tool_use`/`tool_result` (missing backticks) in the `arb_params` doc comment of `tests/client_host_roundtrip.rs` — a file created in plan 01, not modified for this plan's features. With `-D warnings` it fails `make lint`, blocking the gate this plan must pass.
- **Fix:** Backticked the three identifiers in that doc comment.
- **Files modified:** tests/client_host_roundtrip.rs
- **Verification:** `make quality-gate` clears clippy (the whole lint/build/test/doctest/example chain passes).
- **Committed in:** ed5fdaa0 (style follow-up)

**3. [Rule 1 - Bug] clippy::field_reassign_with_default in the new capability tests**
- **Found during:** Phase-end `make quality-gate`
- **Issue:** The capability unit tests built `ClientCapabilities::default()` then reassigned fields — clippy pedantic (`field_reassign_with_default`) rejects this.
- **Fix:** Switched to struct-init syntax `ClientCapabilities { field: .., ..Default::default() }`.
- **Files modified:** src/client/mod.rs
- **Verification:** clippy clean in `make quality-gate`.
- **Committed in:** ed5fdaa0 (style follow-up)

---

**Total deviations:** 3 auto-fixed (2 blocking, 1 bug). **Impact:** No scope creep — all three are correctness/tooling fixes required to deliver a green gate. The empty `[workspace]` is the canonical cargo-fuzz layout; the two clippy fixes are lint-hygiene on this plan's own code plus one pre-existing plan-01 doc line that the current toolchain surfaced.

## Deferred Issues

**D-106-B (logged in `deferred-items.md`): `make quality-gate` purity-check trips on a `quick-xml` version ambiguity — unrelated to this plan.** Everything the gate validates for plan 106-02 passed: `cargo fmt --all --check`, clippy (pedantic+nursery, `-D warnings`), workspace build, **1142 unit tests + property tests + doctests + integration tests**, and example builds. Only the terminal `purity-check` step (`cargo tree -i quick-xml` for `pmcp-workbook-compiler`) failed with *"specification 'quick-xml' is ambiguous"* because the resolved tree now holds two `quick-xml` versions (`0.37.5` and `0.41.0`) from different transitive deps. This plan changed no dependency declaration (verified via `git diff d05d5aba..HEAD -- '**/Cargo.toml'` — only `version =` lines + the fuzz `[workspace]`/`[[bin]]`); `Cargo.lock` is git-ignored so a fresh resolve picked up a newly-published transitive `quick-xml 0.41.0`. Out of scope (unrelated crate; fix is dependency alignment or a version-qualified `cargo tree` spec — Rule 4). Recommended owner: workbook-compiler / build-tooling maintainer.

## Issues Encountered
- Duplex denial + connection-survival needed a pump that answers a first `tools/list` (with an inbound request injected mid-flight), then a SECOND `tools/list` — built on the plan-01 raw-pump convention (the high-level `Server::run` can't answer its own peer request during a tool call, D-106-A). Resolved cleanly.

## Threat Flags
None — no new trust-boundary surface beyond the plan's `<threat_model>`. T-106-05 (denial-of-wallet) is now genuinely mitigated (preflight prevents the LLM call, unit + duplex proven); T-106-06 (capability lie) closed by registry-authoritative derivation; T-106-07 (routing DoS) covered by the real-routing fuzz; T-106-08 (denial tears down connection) and T-106-11 (deny-reason leak) proven by the sanitized `-32603` + duplex connection-survival test.

## Next Phase Readiness
- HOST-04 and HOST-05 complete; the host surface is additive and shippable as pmcp 2.16.0 with the cargo-pmcp scaffold pin in lockstep.
- Phase 108 (`SamplingSource` / `pmcp-agent`) can build on the two-stage approval seam; D-106-A (server loop answering its own peer request during a tool call) remains the tracked prerequisite for the full agent-hosting flow.
- One environmental gate item (D-106-B, `quick-xml` purity-check ambiguity) is logged for the build-tooling owner; it does not affect this plan's deliverables.

## Self-Check: PASSED

- Both created files exist on disk (`tests/client_host_approval.rs`, `fuzz/fuzz_targets/client_host_routing.rs`).
- All 5 commits present in git history (fc9873a3, 69ee6381, 62b0ffbb, ffd50998, ed5fdaa0).
- Acceptance greps hold: `host_registry.approval` + `host_registry.result_review` in `src/client/mod.rs`; `host_registry.(sampling|elicitation|roots)` in the initialize/derivation path; `classify_host_request` in the fuzz target; `2.16.0` in `Cargo.toml` + `workbook_server.rs`.
- Verifications green: `cargo test --lib client`, `cargo test --test client_host_approval`, `cargo test --test client_host_roundtrip`, `cd fuzz && cargo build --bin client_host_routing` (+ `cargo fuzz run ... -runs=2000`), `cargo test -p cargo-pmcp emitted_pmcp_version_matches_workspace_pin`; `make quality-gate` fmt/clippy/build/1142-tests/doctests/examples all pass (only the unrelated D-106-B purity-check step fails).

---
*Phase: 106-client-host-surface*
*Completed: 2026-07-17*
