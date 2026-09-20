---
phase: 117-agents-tester-v1-severability
verified: 2026-08-09T00:46:48Z
status: passed
score: 4/4 must-haves verified
overrides_applied: 0
---

# Phase 117: Agents, Tester & v1 Severability — Verification Report

**Phase Goal:** pmcp's own higher-level clients reach v2 (`pmcp-agent` incl. task polling,
`mcp-tester` for dual-version testing), and v1-only machinery is isolated behind a clearly
severable era-gated layer with a documented sunset policy — so a future major removal is a
deletion, not a refactor — while the v2 path is simplified of session/SSE baggage.

**Verified:** 2026-08-09T00:46:48Z
**Status:** passed
**Re-verification:** No — initial verification

## Verification Method

This phase makes a falsifiable execution claim (v1 machinery is severable at compile time,
and the v2 path carries no session/SSE baggage). Every truth below was checked BY RUNNING
the code — the severed build, the runtime severance proofs, the live agent/server example,
and a live `mcp-tester --dual-run` smoke test — not by reading SUMMARY.md or trusting
self-check narration. `117-REVIEW.md` recorded 2 Critical + 15 Warning findings after
execution; this verification reproduced both Critical defects against the pre-fix state
description, then confirmed the fix commits (`b5d7527f..f6036171`, all present at current
HEAD `f6036171`) actually close them, by direct execution rather than by trusting the
commit messages.

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `pmcp-agent` (incl. `ToolInvoker` and task polling) works end-to-end against a v2 server (CLNT-03) | VERIFIED | `cargo test -p pmcp-agent --test agent_v2_e2e --features url-connector` → 4/4 passed (`agent_reaches_a_v2_server_end_to_end`, `agent_drives_task_polling_to_terminal_on_v2`, `agent_falls_back_to_v1_when_the_server_answers_and_rejects_v2`, `an_unreachable_host_propagates_and_is_not_reported_as_era_v1`). `era_negotiation` 3/3 passed. Live run: `cargo run --example s53_v2_agent_client --features full -- 127.0.0.1:8147` against a live `s47_v2_stateless_mrtr` server exits 0, printing genuine v2 negotiation, v1 fallback, and unreachable-host propagation |
| 2 | `mcp-tester` can exercise a v2 server (headers, discover, stateless flow) for dual-version testing (CLNT-04) | VERIFIED | `cargo test -p mcp-tester --test dual_run --test era_baseline --test report_compat` → 25/25 passed. Live smoke test: `mcp-tester conformance http://127.0.0.1:8147 --dual-run` against a real dual-serving server correctly detects "serves BOTH eras (dual)", runs the suite twice, and reports a genuine baseline-driven era comparison (9 expected differences by design, citing source lines; 1 unexpected; 4 no-longer-reproducing) |
| 3 | v1-only machinery (initialize/session lifecycle, SSE resumability) is isolated behind a clearly severable era-gated layer with a documented sunset policy — future removal is a deletion (SMPL-01) | VERIFIED | `full-v2 = full − v1-compat` exactly, confirmed by reading `Cargo.toml:218/233` (drift is additionally tripwired by `tests/v1_severability_tripwire.rs`, 17/17 passed). `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` compiles. `docs/v1-sunset-policy.md` covers both Server and Client sections, corrected of the WR-03/04/05/06/07/11/12 contradictions found in review. `v1-severance` CI job is wired into `gate.needs` (`.github/workflows/ci.yml:395,533`), confirmed structurally by 8/8 passing tests in `tests/ci_severance_gate_wiring.rs` |
| 4 | The v2 code path carries no session/SSE-resumability baggage, and a simplification pass removes code the v2 model obsoletes (SMPL-02) | VERIFIED | `V1State` is a genuine zero-sized `pub(crate) struct V1State;` on `full-v2` (`v1_session_off.rs:81`) vs. the real session/SSE-stream maps on `v1-compat` (`v1_session.rs:146`) — the structural "no session map allocated" proof. Client transport (`src/shared/streamable_http.rs`) has 68 `v1-compat` gate sites covering `session_id`, `resumption_token`, `on_resumption_token`, `LAST_EVENT_ID`. `./scripts/run-severance-proofs.sh` (the phase's central proof) ran end-to-end, exit 0, with every proof file reporting a genuine non-zero test count (5, 1, 3 for the three runtime proofs; 1867 lib + 525 aggregate-integration tests for the severed test build) |

**Score:** 4/4 truths verified

### Critical Defects From Code Review — Fix Verification

`117-REVIEW.md` (2026-08-08T21:43:17Z, `status: issues_found`) found 2 Critical + 15 Warning +
2 Info findings after Wave 6 execution completed. Both Criticals were reproduced against their
described symptoms and their fixes were independently re-executed here, not trusted from the
fix commit's message:

| ID | Defect | Fix Commit | Verified By |
|----|--------|-----------|-------------|
| CR-01 | `is_initialize_request`/`extract_negotiated_version` were pure classifiers wrongly twinned to `false`/`None`, silently downgrading the outbound `MCP-Protocol-Version` header to `2025-03-26` on a `full-v2` build receiving an `initialize` POST | `b5d7527f` | Read `src/server/streamable_http_server.rs:2255-2280` — both functions are now ungated (moved out of the `v1` pair). Ran the purpose-built regression test `cargo test --test v2_initialize_negotiated_version_header --no-default-features --features full-v2` → 3/3 passed, including `the_initialize_header_agrees_with_the_initialize_body`, the exact assertion that fails if either classifier is re-twinned |
| CR-02 | `cargo test -p pmcp --no-default-features --features full-v2` did not compile (8 errors: `#[cfg(test)]` unit-test blocks referenced items the cut removed) | `e4694a1c` | `cargo test -p pmcp --no-default-features --features full-v2 --lib` → 1867 passed, 0 failed, clean compile. Full `./scripts/run-severance-proofs.sh` aggregate step (`cargo test "${SEVERED[@]}"` with no target filter) also compiled and ran all test targets/examples under `full-v2`, ending with "All severance proofs RAN, with non-zero test counts" |

All 15 Warning-level findings were also spot-checked and confirmed fixed: WR-01 (CI wiring:
`v1-severance` job now runs `./scripts/run-severance-proofs.sh`, which is `gate.needs`-blocking
and pinned by `tests/ci_severance_gate_wiring.rs`, 8/8 passed), WR-02 (tautological
`assert!(!cfg!(...))` test deleted — `v2_client_carries_no_session_on_severed_build` now runs 1
test, not 2), WR-03/WR-04/WR-05/WR-11/WR-12 (doc corrections landed — verified by grep against
the corrected text in `docs/v1-sunset-policy.md`), WR-06 (`InMemoryEventStore` doctest now
wrapped so it names only the ungated `EventStore` trait), WR-07 (`LAST_EVENT_ID` intra-doc link
replaced with a code span — `cargo doc --features full-v2,oauth` no longer reports it), WR-08
(8 `doc(cfg(feature = "v1-compat"))` badges added), WR-09 (tripwire keyword table extended —
`v1_severability_tripwire.rs` now passes 17 tests, up from 15), WR-10 (`Allow: POST, OPTIONS`
header added to the 405 constructor), WR-15 (atomic single-lock config read restored via
`outbound_session_from`). WR-13/WR-14 are correctly recorded as pre-existing, out-of-scope
(moved verbatim, not introduced by this phase). IN-01/IN-02 are informational only.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `Cargo.toml` `full-v2`/`v1-compat` features | D-01/D-02 feature split | VERIFIED | `full-v2` = `full` minus exactly `v1-compat` (line 218 vs 233); `v1-compat` is dependency-free and default-on (`default = ["logging", "v1-compat"]`) |
| `src/server/streamable_http_server/v1_session.rs` + `v1_session_off.rs` | Paired-module severance for server session/SSE machinery | VERIFIED | Real `V1State` (sessions + sse_streams + event_store map) vs. zero-sized twin `pub(crate) struct V1State;` |
| `src/shared/streamable_http.rs` | Client-side severance of session id / resumption token / `Last-Event-ID` | VERIFIED | 68 `v1-compat` gate sites; severed-build test `v2_client_carries_no_session_on_severed_build` passes |
| `src/shared/http_constants.rs` | `LAST_EVENT_ID` gated, doc corrected | VERIFIED | Gated behind `v1-compat`; module doc uses a code span, not a broken intra-doc link |
| `docs/v1-sunset-policy.md` | Documented sunset policy, both Server and Client | VERIFIED | Present, corrected of all WR doc-contradiction findings, references `make test-severance` which exists in `Makefile:320` |
| `scripts/run-severance-proofs.sh` | Central runtime severance proof | VERIFIED | Ran end-to-end, exit 0, non-vacuous (nonzero test counts at every stage) |
| `tests/v1_severability_tripwire.rs` | Derived `full`/`full-v2` drift + null-twin tripwire | VERIFIED | 17/17 passed |
| `tests/ci_severance_gate_wiring.rs` | Structural proof `v1-severance` blocks `gate` | VERIFIED | 8/8 passed |
| `tests/v2_verbs_405_on_severed_build.rs`, `tests/v2_client_carries_no_session_on_severed_build.rs`, `tests/v2_initialize_negotiated_version_header.rs` | Runtime severance proofs | VERIFIED | 5, 1, 3 tests respectively, all passed on `full-v2` |
| `tests/v1_byte_identity_after_cut.rs` | v1 wire bytes unchanged on default build | VERIFIED | 9/9 passed with `--features full` |
| `crates/pmcp-agent/tests/agent_v2_e2e.rs`, `era_negotiation.rs` | CLNT-03 four RED-to-GREEN cases | VERIFIED | 4/4 and 3/3 passed with `--features url-connector` |
| `examples/s53_v2_agent_client.rs` | CLAUDE.md ALWAYS runnable example (CLNT-03) | VERIFIED | Compiled and RUN against a live `s47_v2_stateless_mrtr` server, exit 0, demonstrated all 3 documented behaviours |
| `crates/mcp-tester/baselines/era-deltas.yaml` + `src/era_diff.rs`/`era_observations.rs` | CLNT-04 dual-run baseline | VERIFIED | 25/25 unit/integration tests passed; live `--dual-run` smoke test produced a genuine baseline-cited comparison |
| `fuzz/fuzz_targets/era_deltas_parser.rs` | CLAUDE.md ALWAYS fuzz requirement (D-06) | VERIFIED | File exists, previously built/exercised (artifacts present under `fuzz/artifacts/era_deltas_parser`) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `pmcp-agent`'s `UrlConnectorClientFactory::client_for` | v2/v1 server | live HTTP negotiation + `server/discover` | WIRED | Proven by the live `s53_v2_agent_client` run: v2 negotiated with zero handshake bytes, v1 fallback triggered by an actual rejection, unreachable host propagated as an error |
| `mcp-tester`'s `conformance --dual-run` | `ConformanceRunner::run_dual` | CLI flag → `run_dual` wraps the orchestrator twice | WIRED | Live smoke test: "serves BOTH eras (dual); running the suite twice" printed, followed by a real `DUAL-RUN ERA COMPARISON` block |
| `is_initialize_request`/`extract_negotiated_version` | outbound `MCP-Protocol-Version` header | ungated pure classifiers feeding `compute_outbound_protocol_version` | WIRED | `v2_initialize_negotiated_version_header` 3/3 passed — header agrees with body on `full-v2` |
| `v1-severance` CI job | `gate` aggregate | `needs:` + `env:` + `if` chain | WIRED (structurally) | `tests/ci_severance_gate_wiring.rs` 8/8 passed. Runtime GH-Actions semantics of a failed `needs` job propagating to `gate` were NOT independently observed on a live PR (recorded honestly as `D-117-05-A`, deferred by owner decision — see Notes) |

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|-------------|----------------|--------------|--------|----------|
| CLNT-03 | 117-04, 117-07, 117-10 | `pmcp-agent` works e2e against v2 server, incl. task polling | SATISFIED | See Truth #1 |
| CLNT-04 | 117-03, 117-08, 117-11 | `mcp-tester` exercises v2 server for dual-version testing | SATISFIED | See Truth #2 |
| SMPL-01 | 117-01, 117-05, 117-06, 117-13, 117-14 | v1-only machinery severable, documented sunset policy | SATISFIED | See Truth #3 |
| SMPL-02 | 117-02, 117-06, 117-09, 117-12, 117-13, 117-14 | v2 path carries no session/SSE baggage | SATISFIED | See Truth #4 |

No orphaned requirements: `REQUIREMENTS.md`'s Phase-117 mapping table (lines 1035-1038, 1070)
lists exactly CLNT-03, CLNT-04, SMPL-01, SMPL-02 — identical to the plans' declared
`requirements:` frontmatter and to `ROADMAP.md`'s 4 success criteria.

### Anti-Patterns Found

None. Scanned all phase-touched source/test/doc files
(`streamable_http_server.rs`, `v1_session.rs`, `v1_session_off.rs`, `streamable_http.rs`,
`http_constants.rs`, `event_store.rs`, `client/mod.rs`, `composition/mcp_client.rs`,
`v1-sunset-policy.md`, all severance test files, `run-severance-proofs.sh`,
`pmcp-agent`/`mcp-tester` source and tests) for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER`,
`unimplemented!()`/`todo!()`, and "not implemented"/"coming soon" — zero hits.
`RUSTFLAGS="-D warnings" cargo clippy -p pmcp --lib` clean on both `full-v2` and default
(`v1-compat`) builds. `make doc-check` clean (0 warnings).

Two pre-existing, out-of-scope `unused_imports` warnings in `src/server/auth/jwt.rs` and
`jwt_validator.rs` were observed while building `mcp-tester` — these are documented in
`deferred-items.md` (`DEFERRED-117-14-A`) as untouched by this phase (from Phase 116 auth
hardening) and do not appear under `make lint`'s actual feature scope. Not a Phase 117 gap.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Severed build compiles | `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` | Finished, 0 warnings | PASS |
| Severed lib test build compiles + runs | `cargo test -p pmcp --no-default-features --features full-v2 --lib` | 1867 passed | PASS |
| Runtime severance proofs (nonzero) | `./scripts/run-severance-proofs.sh` | Exit 0, all proof stages nonzero, final line "All severance proofs RAN, with non-zero test counts" | PASS |
| CR-01 regression pinned | `cargo test --test v2_initialize_negotiated_version_header --no-default-features --features full-v2` | 3 passed | PASS |
| pmcp-agent CLNT-03 4 cases | `cargo test -p pmcp-agent --test agent_v2_e2e --features url-connector` | 4 passed | PASS |
| Live agent example vs live v2 server | `cargo run --example s53_v2_agent_client --features full -- 127.0.0.1:8147` (paired with a live `s47_v2_stateless_mrtr`) | Exit 0, all 3 demos correct | PASS |
| Live `mcp-tester --dual-run` vs live dual-serving server | `mcp-tester conformance http://127.0.0.1:8147 --dual-run` | Detected dual era, ran suite twice, produced a genuine baseline comparison | PASS |
| `full-v2` rustdoc, matching `make doc-check`'s exact non-v1-compat feature list | `RUSTDOCFLAGS="-D warnings" cargo doc -p pmcp --no-default-features --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket --no-deps` | 0 warnings | PASS |
| `make doc-check` (the real, currently-shipping gate) | `/usr/bin/make doc-check` | 0 warnings | PASS |
| No regression to default (v1-compat) build | `cargo test -p pmcp --lib --features full` | 1880 passed | PASS |
| Full workspace still builds | `cargo build --workspace` | 0 crates to recompile, success | PASS |

### Probe Execution

Not applicable — this phase's central "probe" is `scripts/run-severance-proofs.sh`, executed
above under Behavioral Spot-Checks with full transcript captured
(`/private/tmp/.../scratchpad/severance_full.log`), exit 0.

### Human Verification Required

None. Every truth in this phase was verifiable by direct execution (compiled builds, runtime
test suites, and two live end-to-end demonstrations against real running servers). No UI,
real-time, or subjective-quality claims are part of this phase's goal.

## Notes

- **D-117-05-A (recorded in `deferred-items.md`, not a gap):** the plans' own author honestly
  recorded that the GitHub Actions runtime semantics of "a failed `needs` job produces a
  failed `gate` conclusion" were not observed on a live PR at execution time (no open PR
  existed for this branch). The `if: always()` + `env:`-bound `needs.v1-severance.result`
  pattern used is identical to the pattern already used for `test`, `quality-gate`,
  `purity-check`, `pmcp-agent-targets`, and `wasm32-purity` in the same `gate` job, which is a
  long-standing, previously-exercised mechanism in this repo — not new risk introduced by this
  phase. Not treated as a gap.
- **LIM-117-08-GATE (recorded in `deferred-items.md`, not a gap):** `make quality-gate` does
  not compile or run `mcp-tester`'s tests. This phase's CLNT-04 claim was independently
  verified here by direct `cargo test -p mcp-tester` execution and a live `--dual-run` smoke
  test, so the goal is met even though the Makefile-driven quality gate does not (yet) cover
  this crate. Recorded as a known follow-up, owner unassigned.
- Two pre-existing `unused_imports` warnings in Phase-116 auth files were observed but are
  explicitly out of this phase's scope (confirmed via `git diff --name-only` empty for the
  plans that touch `mcp-tester`).

---

_Verified: 2026-08-09T00:46:48Z_
_Verifier: Claude (gsd-verifier)_
