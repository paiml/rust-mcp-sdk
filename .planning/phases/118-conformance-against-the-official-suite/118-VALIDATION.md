---
phase: 118
slug: conformance-against-the-official-suite
status: approved
nyquist_compliant: true
wave_0_complete: false
created: 2026-08-09
updated: 2026-08-09
---

# Phase 118 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Populated from the 9 committed plans, not from a template.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` / `#[tokio::test]`, `proptest` 1.7, `cargo-fuzz` (libfuzzer), plus the external Node CLI `@modelcontextprotocol/conformance` as the CONF-01 referee |
| **Config file** | None for Rust. `conformance/package.json` + `conformance/package-lock.json` are **Wave 1** (plan 118-02); the suite binary appears only after `npm ci --prefix conformance` |
| **Runtime prerequisite** | Node >= 22 (the suite imports `globSync` from `node:fs` at module scope). Locally: `PATH=~/.nvm/versions/node/v22.22.2/bin:$PATH`. In CI: `actions/setup-node@v4` |
| **Quick run command** | `cargo test -p pmcp-team-servers --test conformance` (CONF-02/03) · `cargo test --features full --test v2_mcp_name_name_bearing_only` (CONF-01 gate) |
| **Full suite command** | `make test-conformance` + `make test-era-matrix` (both created by plan 118-08), then `make quality-gate` |
| **Estimated runtime** | quick: 25-60 s · `make test-conformance`: 10-20 min (npm ci + cold example build + two requirement-set runs) · `make quality-gate`: 8-20 min |

**Selector rule (standing):** `cargo nextest -E 'binary(<name>)'`, NEVER `test(/<name>/)`. The latter
silently selects ZERO tests and exits 0; it bit Phase 114 seven times.

**Vacuity rule (standing):** every runtime claim asserts a line matching `^running [1-9][0-9]* tests?$`
from OUTSIDE the compilation unit. A `cfg!`-based guard inside a `#![cfg]`-selected file expands to
`!false` and cannot fail.

---

## Sampling Rate

- **After every task commit:** the task's own `<automated>` command (all 27 tasks carry one).
- **After every plan wave:**
  - Waves 1-3: `cargo test -p pmcp-team-servers --test conformance` and
    `cargo test -p pmcp-team-servers --test era_baseline` — **explicitly**, because `make quality-gate`
    is scoped to the root `pmcp` package and does NOT reach `crates/pmcp-team-servers/tests/`.
  - Waves 4-5: `make test-conformance` and `make test-era-matrix`.
- **Before `/gsd:verify-work`:** both requirement sets exit 0 from ONE process, `gate` green on the PR,
  `make quality-gate` exit 0.
- **Max feedback latency:** 60 s for the per-task Rust commands; 20 min for the capstone gates. The
  capstone gates are acceptance checks, not inner-loop checks — see the latency note in each plan.

---

## Per-Task Verification Map

27 tasks across 9 plans. Every task has an `<automated>` command — coverage is **27/27 = 100%**.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 118-01-01 | 01 | 1 | CONF-01 | T-118-01/02/04 | `Mcp-Name` required exactly on name-bearing methods; `Mcp-Method` + version still mandatory; bounded header reads retained | unit | `cargo test --features full --lib server::streamable_http_server` | ✅ exists | ⬜ pending |
| 118-01-02 | 01 | 1 | CONF-01 | T-118-01/02 | Mismatched or absent `Mcp-Name` on a name-bearing method is rejected over real HTTP | unit + property + integration | `cargo test --features full --test v2_mcp_name_name_bearing_only` | ❌ W1 (created by this task) | ⬜ pending |
| 118-01-03 | 01 | 1 | CONF-01 | T-118-03 | No stale restatement of the fail-closed rule survives | repo sweep + gate | `make quality-gate` | ✅ exists | ⬜ pending |
| 118-02-01 | 02 | 1 | CONF-01 | T-118-05/06/07 | Exact-pinned, integrity-hashed, postinstall-free suite install | integration | `rm -rf conformance/node_modules && npm ci --prefix conformance && npm ls --prefix conformance --depth=0` | ❌ W1 | ⬜ pending |
| 118-02-02 | 02 | 1 | CONF-01 | T-118-08/09 | CI tooling never reaches the crates.io tarball | CLI assertion | `cargo package -p pmcp --list --allow-dirty \| grep -E 'conformance/\|ci_conformance_gate_wiring'` (must match nothing) | ✅ exists | ⬜ pending |
| 118-02-03 | 02 | 1 | CONF-01 | T-118-07 | Re-pin procedure and forbidden flags are documented | doc assertion | `grep -q -- '--requirements' conformance/README.md && grep -q 'globSync' conformance/README.md` | ❌ W1 | ⬜ pending |
| 118-03-01 | 03 | 1 | CONF-02, CONF-03 | T-118-10/14 | Baseline parser is total; duplicate/empty join keys rejected | unit + property | `cargo test -p pmcp-team-servers --lib conformance::era_baseline` | ❌ W1 | ⬜ pending |
| 118-03-02 | 03 | 1 | CONF-02, CONF-03 | T-118-12 | Every expected difference carries a checkable citation | unit (via compiled-in parse) | `cargo test -p pmcp-team-servers --lib conformance::era_baseline` | ❌ W1 | ⬜ pending |
| 118-03-03 | 03 | 1 | CONF-02, CONF-03 | T-118-10/11/13 | Shrunk baseline, drifted protocol pin, or uncited source all fail | unit + fuzz | `cargo test -p pmcp-team-servers --test era_baseline` (+ `cargo +nightly fuzz run team_era_deltas_parser -- -max_total_time=60`) | ❌ W1 | ⬜ pending |
| 118-04-01 | 04 | 2 | CONF-01 | T-118-15/16/17 | Loopback-only bind, no secret in the banner, live session path retained | build | `RUSTFLAGS="-D warnings" cargo build --example s54_v2_dual_conformance --features full` | ❌ W2 | ⬜ pending |
| 118-04-02 | 04 | 2 | CONF-01 | T-118-19 | Every scored resource/prompt scenario passes with a NONZERO check count | build + suite scenarios | `RUSTFLAGS="-D warnings" cargo build --example s54_v2_dual_conformance --features full` | ❌ W2 | ⬜ pending |
| 118-04-03 | 04 | 2 | CONF-01 | T-118-19 | The 2025-11-25 scored set exits 0; no scenario passes on zero checks | integration (suite) | `RUSTFLAGS="-D warnings" cargo build --example s54_v2_dual_conformance --features full && make quality-gate` | ❌ W2 | ⬜ pending |
| 118-05-01 | 05 | 3 | CONF-01 | T-118-23/24/25 | Zero `Mcp-Name` rejections remain; every scored failure classified before code is written | measurement | `test -d results/v2-premeasure && ls results/v2-premeasure` | ❌ W3 | ⬜ pending |
| 118-05-02 | 05 | 3 | CONF-01 | T-118-20/21 | Tampered sealed request state rejected by the SDK, not by bespoke example logic | build + suite scenarios | `RUSTFLAGS="-D warnings" cargo build --example s54_v2_dual_conformance --features full` | ❌ W3 | ⬜ pending |
| 118-05-03 | 05 | 3 | CONF-01 | T-118-22/24 | Both requirement sets exit 0 from ONE unrestarted process | integration (suite ×2) | `RUSTFLAGS="-D warnings" cargo build --example s54_v2_dual_conformance --features full && make quality-gate` | ❌ W3 | ⬜ pending |
| 118-06-01 | 06 | 2 | CONF-02 | T-118-30 | Prose-only rename; fixture data byte-identical; contract bindings intact | integration | `cargo test -p pmcp-team-servers --test conformance` | ✅ exists | ⬜ pending |
| 118-06-02 | 06 | 2 | CONF-02 | T-118-26/28/31 | Unlisted difference FAILS and stale entry FAILS; a zero-observation matrix cannot pass | integration | `cargo test -p pmcp-team-servers --test conformance` | ✅ exists | ⬜ pending |
| 118-06-03 | 06 | 2 | CONF-02 | T-118-27 | Baseline reconciled from measurement; floor raised, never lowered | unit + integration | `cargo test -p pmcp-team-servers --test era_baseline && cargo test -p pmcp-team-servers --test conformance && make quality-gate` | ❌ W1 (118-03) | ⬜ pending |
| 118-07-01 | 07 | 3 | CONF-03 | T-118-35/37 | Deterministic target; no `#[deprecated]`, no `warn!`, no runtime signal | build + unit | `RUSTFLAGS="-D warnings" cargo build -p pmcp-team-servers --all-features && cargo test -p pmcp-team-servers --lib conformance::deprecated_caps` | ❌ W3 | ⬜ pending |
| 118-07-02 | 07 | 3 | CONF-03 | T-118-32/33/34 | Roots/Sampling/Logging COMPLETE under both eras via their era mechanism | integration | `cargo test -p pmcp-team-servers --test conformance` | ❌ W3 | ⬜ pending |
| 118-07-03 | 07 | 3 | CONF-03 | T-118-35/36 | Policy states the window without contradicting the removal condition | doc assertion + gate | `grep -q '12-month' docs/v1-sunset-policy.md && grep -q 'deprecated-caps' docs/v1-sunset-policy.md && make quality-gate` | ✅ exists | ⬜ pending |
| 118-08-01 | 08 | 4 | CONF-01 | T-118-38/39/40/41/42/43 | One process, two runs, trap teardown, no orphaned port, no allowlist | integration (script) | `bash -n scripts/run-conformance-suite.sh && ./scripts/run-conformance-suite.sh` | ❌ W4 | ⬜ pending |
| 118-08-02 | 08 | 4 | CONF-02 | T-118-39/44 | Dev-dependency-free build fence + nonzero-test-count guard outside the compilation unit | integration (script) | `bash -n scripts/run-era-matrix.sh && ./scripts/run-era-matrix.sh` | ❌ W4 | ⬜ pending |
| 118-08-03 | 08 | 4 | CONF-01, CONF-02 | — | Local spelling exists and is absent from `quality-gate` | CLI assertion | `make -n test-conformance && make -n test-era-matrix` | ✅ exists | ⬜ pending |
| 118-09-01 | 09 | 5 | CONF-01, CONF-02, CONF-03 | T-118-49/50/51 | Node 22 pinned, results uploaded on green, no advisory escape hatch, no elevated token | structural | `grep -c '^  conformance-suite:' .github/workflows/ci.yml; cargo test --features full --test ci_severance_gate_wiring` | ✅ exists | ⬜ pending |
| 118-09-02 | 09 | 5 | CONF-01, CONF-02, CONF-03 | T-118-45/46 | Every awaited job is bound, read and named in the failure echo | structural | `grep -A 32 '^  gate:' .github/workflows/ci.yml \| grep -cE 'conformance-suite\|era-matrix\|CONFORMANCE_RESULT\|ERA_MATRIX_RESULT'` | ✅ exists | ⬜ pending |
| 118-09-03 | 09 | 5 | CONF-01, CONF-02, CONF-03 | T-118-45/47/48/52 | Wiring proved by parsing, with a live negative control and four executed failure demos | unit (structural) | `cargo test --features full --test ci_conformance_gate_wiring && cargo test --features full --test ci_severance_gate_wiring` | ❌ W5 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

**Sampling continuity:** no 3 consecutive tasks lack an automated verify — the longest run without
one is 0.

---

## Wave 0 Requirements

There is no separate Wave 0. Every missing test artifact is created by the plan that first needs it,
and each is listed as `❌ W<n>` above. For clarity, the artifacts that do not exist today:

- [ ] `conformance/package.json` + `conformance/package-lock.json` — 118-02 Task 1 (Wave 1). **Blocks
      118-04 and 118-08**; this is why 118-04 declares `depends_on: ["118-02"]` and sits in Wave 2.
- [ ] `tests/v2_mcp_name_name_bearing_only.rs` — 118-01 Task 2 (Wave 1)
- [ ] `crates/pmcp-team-servers/src/conformance/era_baseline.rs` +
      `crates/pmcp-team-servers/baselines/era-deltas.yaml` +
      `crates/pmcp-team-servers/tests/era_baseline.rs` — 118-03 (Wave 1)
- [ ] `fuzz/fuzz_targets/team_era_deltas_parser.rs` + its `[[bin]]` registration — 118-03 Task 3 (Wave 1)
- [ ] `examples/s54_v2_dual_conformance.rs` — 118-04 (Wave 2)
- [ ] `crates/pmcp-team-servers/src/conformance/deprecated_caps.rs` +
      `contracts/team-servers/fixtures/deprecated-caps/` — 118-07 (Wave 3)
- [ ] `scripts/run-conformance-suite.sh` + `scripts/run-era-matrix.sh` — 118-08 (Wave 4)
- [ ] `tests/ci_conformance_gate_wiring.rs` — 118-09 Task 3 (Wave 5)

Existing infrastructure that needs no Wave 0 work: `crates/pmcp-team-servers/tests/conformance.rs`
(9 tests today), `tests/common/v2.rs` (spawn/probe harness), `tests/ci_severance_gate_wiring.rs`
(the structural-proof template), `scripts/run-severance-proofs.sh` (the script template).

---

## Manual-Only Verifications

All phase behaviours have automated verification. The items below are **human-reviewed evidence**
recorded in SUMMARYs, not substitutes for an automated check — each sits alongside a passing
automated command in the map above.

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Package legitimacy at (re-)pin time | CONF-01 | A registry verdict is a point-in-time human judgement, not a repeatable assertion | `slopcheck install -e npm @modelcontextprotocol/conformance` (the `-e npm` is mandatory in this Rust repo) + `npm view ... scripts.postinstall` + `npm view ... dist-tags`; record all three verbatim (118-02 Task 1) |
| Baseline entries read as a spec | CONF-02, CONF-03 | "A reviewer can check this citation" is a judgement; only the >= 10-char floor is mechanical | Read each `source:` and record the entries as a table in the SUMMARY (118-03 Task 2, 118-06 Task 3) |
| Negative controls (12 across the phase) | all | Proving a gate CAN fail requires temporarily breaking it, which cannot live in the committed suite | Each plan names its controls and requires the failure message verbatim in the SUMMARY: 118-01 ×1, 118-03 ×3, 118-06 ×2, 118-07 ×1, 118-08 ×4, 118-09 ×4 |
| Classification of scored v2 failures | CONF-01 | Deciding "missing fixture" vs "SDK gap" is the judgement the phase exists to surface | 118-05 Task 1: classify every scored failing check (a)/(b)/(c)/(d); class (c) HALTS the plan with a `## FINDING` |
| Policy self-consistency | CONF-03 | Whether two prose claims contradict is not grep-checkable | 118-07 Task 3: quote the reconciliation sentence in the SUMMARY; `git diff` asserts the removal-condition clause is unchanged |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies — 27/27
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (each listed with its creating plan and wave)
- [x] No watch-mode flags
- [x] Feedback latency < 60 s for per-task commands; capstone gates are acceptance checks and carry
      an explicit latency note in each plan so slowness is not misread as a hang
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-08-09
