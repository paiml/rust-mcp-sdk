---
phase: 117
slug: agents-tester-v1-severability
status: planned
nyquist_compliant: true
wave_0_complete: false
created: 2026-08-07
---

# Phase 117 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from `117-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Built-in `cargo test` (libtest) + `proptest 1.7` + `quickcheck 1.0` |
| **Config file** | none — `[dev-dependencies]` at `Cargo.toml:180-201`; `tests/` is the integration root (124 entries) |
| **Quick run command** | `cargo test --test <name> --features "full"` |
| **Full suite command** | `make test-all` (`Makefile:369` → `test-unit test-doc test-property test-examples test-integration`) |
| **Blocking CI gate** | `make quality-gate` at `ci.yml:233-234`, inside the `quality-gate` job, which IS in `gate.needs` (`ci.yml:443`) |
| **Estimated runtime** | quick ~30s · full suite several minutes |

### ⚠ Two infrastructure landmines that invalidate results silently

1. **nextest selector.** `cargo-nextest` is installed in CI (`ci.yml:208-213`) but `make quality-gate`
   uses plain `cargo test`. Any nextest command in a plan MUST use `binary(<name>)`, never
   `test(/pattern/)` — the latter silently selects **zero** tests and still exits 0. This has bitten
   this project repeatedly (7× in Phase 114 alone).
2. **`make quality-gate` alone does NOT prove severance.** It runs `--all-features`
   (`Makefile:135`), which enables `v1-compat` **and** `full-v2` simultaneously. The severance build
   must be run and reported **separately, every wave**.

---

## Sampling Rate

- **After every task commit:** `cargo test --test <the one test this task adds> --features "full"`
  plus `cargo fmt --all -- --check`. Target < 30s.
- **After every plan wave:**
  - **Wave 1** (feature + tripwire + CI): `cargo test --test v1_severability_tripwire` +
    `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2`
  - **Wave 2** (the transport cut): the Wave 1 set **plus** `cargo test --lib --features "full"`
    (the 1,851-line in-file test module is the primary regression net) **plus**
    `cargo test --test v1_byte_identity_after_cut --features "full"`
  - **Wave 3** (agent + tester): `cargo test -p pmcp-agent` + `cargo test -p mcp-tester` +
    `cargo build -p cargo-pmcp` + `cargo build -p pmcp-agent --target wasm32-unknown-unknown`
- **Before `/gsd:verify-work`:** `make quality-gate` green, **plus** the severance build, **plus**
  `make doc-check`, **plus** the adversarial CI-blocking check.
- **Max feedback latency:** 30 seconds per task; wave gates may run longer.

---

## Per-Task Verification Map

> Task IDs are assigned by the planner. This table is the requirement→proof contract the plans must
> satisfy; the planner fills `Task ID` / `Plan` columns when plans are written.

| Task ID | Plan | Wave | Requirement | Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|----------|-----------|-------------------|-------------|--------|
| 117-01 T2 | 117-01 | 1 | SMPL-01 | `full` and `full-v2` differ by exactly `v1-compat`, **derived** from `Cargo.toml` | unit | `cargo test --test v1_severability_tripwire` | ❌ W0 | ⬜ pending |
| 117-01 T2 | 117-01 | 1 | SMPL-01 | `v1-compat` is present in BOTH `default` and `full` | unit | `cargo test --test v1_severability_tripwire` | ❌ W0 | ⬜ pending |
| 117-01 T1 / 117-05 T1 | 117-01, 117-05 | 1 | SMPL-01 / SMPL-02 | Crate compiles with the **real transport** and NO v1 layer | build | `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` | ❌ W0 (new `ci.yml` job) | ⬜ pending |
| 117-05 T2+T3 | 117-05 | 1 | SMPL-01 | The severance job is reachable from `gate.needs` and actually blocks | **manual-only** | Scratch PR that breaks `full-v2`; assert `gate` reports failure | ❌ manual by necessity | ⬜ pending |
| 117-06 T3 | 117-06 | 2 | SMPL-02 | The `full-v2` build contains no session/SSE-resumability code | unit (source tripwire) | `cargo test --test v1_severability_tripwire` — assert **SEMANTICALLY**: `V1State` is a unit struct, no state-bearing type (`HashMap`/`RwLock`/`EventStoreHandle`) is held, no state/header **operation** is performed, and the twin declares nothing `v1_session.rs` does not (derived, not enumerated). **A substring blacklist is forbidden** — four of the eight original tokens (`sessions`, `event_store`, `EventStore`, `sse_streams`) are provably unsatisfiable against identifiers 117-09 requires. Guarded by a non-vacuity floor plus one POSITIVE control (`sessions_active_for` must be ACCEPTED) | ❌ W0 | ⬜ pending |
| 117-02 T1+T2 (capture) / 117-09,12,13 (assert) | 117-02, 117-09, 117-12, 117-13 | 2 | SMPL-02 | The v1 wire is **byte-identical** across the cut | integration (golden) | `cargo test --test v1_byte_identity_after_cut --features "full"` | ❌ W0 — **capture goldens BEFORE the cut** | ⬜ pending |
| 117-09 T2 | 117-09 | 2 | SMPL-02 | `sessions_active` / `resumability_active` truth tables survive the move | unit + property | `cargo test --lib --features "full" sessions_active` / `resumability_active` | ✅ `streamable_http_server.rs:4568,4585,5879,5894` | ⬜ pending |
| 117-12 T2 | 117-12 | 2 | SMPL-02 | v2 exchanges write zero event-store traffic and replay nothing | integration (spy) | `cargo test --lib --features "full" spy_records` | ✅ `streamable_http_server.rs:5946,5972,5997,6015` | ⬜ pending |
| 117-09 T1 / 117-12 T2 | 117-09, 117-12 | 2 | SMPL-02 | A v2 response is never routed into a session SSE stream | integration | `cargo test --lib --features "full" v2_response_is_never_routed` | ✅ `streamable_http_server.rs:6060` | ⬜ pending |
| 117-01 T3 / 117-13 T3 | 117-01, 117-13 | 1 / 6 | SMPL-01 | The sunset-policy rustdoc compiles warning-free | doc | `make doc-check` (after adding `v1-compat` to `Makefile:429`) | ✅ target exists; ❌ feature-list edit | ⬜ pending |
| 117-13 T2 | 117-13 | 6 | SMPL-02 | GET and DELETE return **405** on the severed build — a RUNTIME claim proven by a RUN, not by a build | integration (severed build) | `cargo test --test v2_verbs_405_on_severed_build --no-default-features --features full-v2` — a `0 tests` result is a FAILURE; 404-vs-405 negative control discriminates routed-and-405 from unrouted | ❌ W0 | ⬜ pending |
| 117-14 T1 | 117-14 | 5 | SMPL-01 / SMPL-02 | The client's SSE-resumability surface (`resumption_token`, `on_resumption_token`, `Last-Event-ID` writer) is gated; A4 measured | build + source | `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` && `cargo build -p pmcp --no-default-features --features "streamable-http"` | ❌ W0 — 55 ungated v1 sites measured in `src/shared/streamable_http.rs` | ⬜ pending |
| 117-14 T2 | 117-14 | 5 | SMPL-01 / SMPL-02 | The client session lifecycle (`session_id` capture, DELETE teardown) is gated; `Client::initialize` severability MEASURED not assumed | build + source | `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` && `cargo test --lib --features "full"` | ❌ W0 — `streamable_http.rs:946` capture, `:1412-1428` teardown | ⬜ pending |
| 117-14 T3 | 117-14 | 5 | SMPL-01 | The client inventory is **derived** (not enumerated) and proven on the severed build at RUNTIME | unit (tripwire) + integration (severed build) | `cargo test --test v1_severability_tripwire` && `cargo test --test v2_client_carries_no_session_on_severed_build --no-default-features --features full-v2` | ❌ W0 | ⬜ pending |
| 117-04 T2 / 117-07 T1 | 117-04, 117-07 | 3 | CLNT-03 | Agent connects to a v2 server end-to-end (tools/list → tools/call → task poll → terminal) | integration (live socket) | `cargo test --test agent_v2_e2e --features "full"` | ❌ W0 | ⬜ pending |
| 117-04 T2 / 117-07 T1 | 117-04, 117-07 | 3 | CLNT-03 | Agent **falls back to v1** when the server answers-and-rejects v2 | integration (live socket) | `cargo test --test agent_v2_e2e --features "full" fallback` | ❌ W0 — **the D-07 negative case; must not be skipped** | ⬜ pending |
| 117-04 T2 / 117-07 T1 | 117-04, 117-07 | 3 | CLNT-03 | An unreachable host **propagates** rather than reporting era V1 | integration | `cargo test --test agent_v2_e2e --features "full" unreachable` | ❌ W0 | ⬜ pending |
| 117-07 T2 | 117-07 | 3 | CLNT-03 | A pre-117 `EffectTrace` (no era field) still deserializes; a `None` era serializes byte-identically | unit | `cargo test -p pmcp-agent trace` | ✅ harness `trace.rs:221-227`; new cases needed | ⬜ pending |
| 117-07 T3 | 117-07 | 3 | CLNT-03 | `ReplayInvoker` fails deterministically on an era mismatch | property | `cargo test -p pmcp-agent --test replay_safety` | ✅ `tests/replay_safety.rs` (AGNT-03); new case needed | ⬜ pending |
| 117-07 T1 | 117-07 | 3 | CLNT-03 | `pmcp-agent` still builds for wasm32 under default features | build | `cargo build -p pmcp-agent --target wasm32-unknown-unknown` | ✅ `ci.yml:374-377` (in `gate.needs`) | ⬜ pending |
| 117-11 T3 | 117-11 | 3 | CLNT-04 | Dual-run detects a both-era server and emits a comparison | integration (live socket) | `cargo test -p mcp-tester --test dual_run` | ❌ W0 | ⬜ pending |
| 117-03 T1+T2 (capture) / 117-11 (re-assert) | 117-03, 117-11 | 3 | CLNT-04 | Single-run stdout is **BYTE-IDENTICAL** to 0.7.0 for both `--format pretty` and `--format json` | integration (golden) | `cargo test -p mcp-tester --test report_compat` | ❌ W0 — the D-11 additivity proof | ⬜ pending |
| 117-11 T1 | 117-03, 117-08, 117-11 | 3 | CLNT-04 | `cargo-pmcp` still compiles against the changed `mcp-tester` | build | `cargo build -p cargo-pmcp` | ✅ implicit via `make build` — make it an explicit acceptance item | ⬜ pending |
| 117-08 T2 | 117-08 | 3 | CLNT-04 | Every baseline entry has a unique id and a non-empty `source`; count ≥ 14 | unit | `cargo test -p mcp-tester --test era_baseline` | ❌ W0 | ⬜ pending |
| 117-08 T3 | 117-08 | 1/3 | ALWAYS (CLAUDE.md) | Fuzz target for the baseline / feature-list parser | fuzz | `cargo fuzz run <target>` | ❌ W0 | ⬜ pending |
| 117-10 T1+T2 | 117-10 | 3 | ALWAYS (CLAUDE.md) | Runnable example: agent against a v2 server | example | `cargo run --example s53_v2_agent_client --features "full"` | ❌ W0 (follows `s47`/`s48` precedent) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] **A1 resolution spike — CLOSED 2026-08-07, before planning.** `cargo tree -p pmcp
      --no-default-features -e features` yields **0** axum nodes although
      `crates/pmcp-server-toolkit/Cargo.toml:125` enables `pmcp/streamable-http`; the same tree with
      `--features streamable-http` yields 25. `-p pmcp` does not unify features with siblings, so the
      severance build is not a false green. **The D-02 mechanism is sound — this no longer blocks.**
- [ ] `tests/v1_severability_tripwire.rs` — **derived** `full`/`full-v2` drift + `default`
      membership + v1-module source-content check, all with non-vacuity guards (SMPL-01, SMPL-02).
      Scope MUST be derived from `Cargo.toml` at test time, never enumerated (116-14 precedent).
- [ ] `tests/v1_byte_identity_after_cut.rs` — **golden v1 wire fixtures captured BEFORE the cut**
      (initialize response, session header emission, `Last-Event-ID` replay). Capturing them after
      the cut proves nothing (SMPL-02).
- [ ] New `ci.yml` job `v1-severance` + the **three** `gate` edits (`:443`, `:447-452`, `:454-458`).
      One edit is not enough — `gate` evaluates named env vars explicitly (SMPL-01).
- [ ] `Makefile:429` — add `v1-compat` to the `doc-check` feature list (SMPL-01).
- [ ] `docs/v1-sunset-policy.md` (SMPL-01, D-04).
- [ ] `crates/mcp-tester/baselines/era-deltas.yaml (YAML chosen: `serde_yaml` is already a dep; `toml` is NOT)` — 14 seeded entries, each with `source`
      and `provisional` (CLNT-04).
- [ ] `crates/mcp-tester/tests/era_baseline.rs` — baseline schema + non-vacuity tripwire (CLNT-04).
- [ ] `crates/mcp-tester/tests/report_compat.rs` — golden single-run stdout for `pretty` and `json`,
      captured against **0.7.0 as it stands today** (CLNT-04, D-11/A-D11).
- [ ] `crates/mcp-tester/tests/dual_run.rs` — live-socket dual-run against the in-repo v2 example
      (CLNT-04).
- [ ] `crates/pmcp-agent/tests/agent_v2_e2e.rs` — v2 happy path, **v1 fallback**, unreachable-host
      propagation (CLNT-03, D-07).
- [ ] `examples/s53_v2_agent_client.rs` (next free number — `s49` is occupied twice; `s50`/`s51`/`s52` exist) — CLAUDE.md ALWAYS runnable example
      (CLNT-03).
- [ ] A fuzz target for the feature-list / baseline parser — CLAUDE.md ALWAYS fuzz requirement.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| The `v1-severance` CI job actually **blocks merge** | SMPL-01 | CI blocking semantics cannot be asserted from inside the repo. This is the `CORRECTION-116-DOC` rule verbatim: prove a gate blocks from the **workflow file**, not the Makefile — and the existing `feature-flags` job (`ci.yml:141-164`) is live proof of the trap, being absent from `gate.needs` (`ci.yml:443`). | Open a scratch PR that deliberately breaks the `full-v2` build (e.g. reference a `v1-compat`-gated symbol from ungated code). Confirm the `v1-severance` job fails **and** that the aggregate `gate` check reports failure. Revert the scratch PR. Record the observed `gate` conclusion in the plan's evidence. |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or a Wave 0 dependency — 36 tasks, 37 `<automated>` blocks (every task carries one; 117-05 adds a 4th at plan level for the manual branch-protection check)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify — every task has one
- [x] Wave 0 covers all ❌ MISSING references above — all new files are created in Wave 1/2 plans before the plan that asserts against them
- [x] No watch-mode flags — grepped, zero
- [x] Any nextest command uses `binary(...)`, never `test(/.../)` — zero nextest commands appear in any plan; all verify blocks use plain `cargo test`, which is what `make quality-gate` runs
- [x] Severance build reported **separately** from `make quality-gate` every wave — stated in the `<verification>` block of 117-01, 117-06, 117-09, 117-10, 117-12, 117-13 and 117-14
- [x] Feedback latency < 30s per task — except the three live-socket suites (117-02, 117-04, 117-11), each of which carries an explicit runtime bound as an acceptance criterion
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** plans written 2026-08-07 — **14 plans, 6 waves** (revised 2026-08-07 from cross-AI
review, commit `d08aad05`: plan 117-14 added for client-side severance at wave 5; 117-13 moved to
wave 6); every row above maps to a real task
