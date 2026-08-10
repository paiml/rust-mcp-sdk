---
phase: 118-conformance-against-the-official-suite
verified: 2026-08-10T02:35:54Z
status: passed
score: 3/3 must-haves verified
overrides_applied: 0
---

# Phase 118: Conformance Against the Official Suite — Verification Report

**Phase Goal:** The dual-version claim is validated by construction — the official conformance
suite plus the extended Rust harness both run against whatever the dual-version binary actually
does, with v1 fixtures kept green and deprecated capabilities verified still-functional under v2.
**Verified:** 2026-08-10T02:35:54Z
**Status:** passed
**Re-verification:** No — initial verification

## Normative context applied

`118-CONFORMANCE-GAPS.md` (D-21) was read first and treated as authoritative. Per D-21, the
official suite genuinely does not exit 0 on either requirement set (measured: 51/15 v1, 124/54
v2) because of nine cited, source-verified structural SDK gaps (G-1..G-9). The phase's decision
was to scope the *blocking* gate to surfaces that measure entirely green (MRTR + check floors +
zero-check set equality) and state the rest in writing rather than suppress it. This verification
does **not** treat "full suite exits 0" as a must-have — it verifies (a) the scoped gate is real
and genuinely blocking, and (b) no suppression mechanism was smuggled in anywhere.

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | The official `@modelcontextprotocol/conformance` suite (pinned to a commit, re-pinned after the final spec) runs in CI against a dual-version pmcp server example over real HTTP (CONF-01) | ✓ VERIFIED | Independently **re-executed live** (not from SUMMARY): installed Node 22, ran `scripts/run-conformance-suite.sh` end to end against `target/debug/examples/s54_v2_dual_conformance`. Reproduced the exact D-21 numbers byte-for-byte: `2025-11-25` → 33 scenario dirs, 66 checks (floor 66) OK, suite total 51 passed/15 failed; `2026-07-28` → 50 scenario dirs, 178 checks (floor 178) OK, suite total 124 passed/54 failed. MRTR surface: 14/14 scenarios, 36 checks, **0 failures**. Zero-check gate: exact match both directions. Script printed `CONF-01 gates PASSED.` One process, one PID, both runs (D-06 claim independently confirmed from the script's own liveness assertions). |
| 2 | The Phase-109 Rust conformance harness gains v2 fixtures while v1 fixtures stay green (dual conformance), verified with a dev-dependency-free build to avoid feature-unification false-greens (CONF-02) | ✓ VERIFIED | Independently **re-executed live**: ran `scripts/run-era-matrix.sh` end to end. Both dev-dependency-free build fences passed (`--all-features`, `--no-default-features --features conformance`, both under `RUSTFLAGS="-D warnings"`). All three harness targets ran with non-zero test counts: `conformance` 10 passed/0 failed/1 (deliberately) ignored — the 33-case v1 fixture corpus stays green; `era_matrix --features http` 4 passed/0 failed; `era_baseline --features http` 10 passed/0 failed. |
| 3 | Deprecated Roots/Sampling/Logging capabilities remain fully functional under v2 negotiation (advisory-only deprecation, 12-month window) (CONF-03) | ✓ VERIFIED | `era_matrix.rs::deprecated_capabilities_complete_under_both_eras` (passed live, above) drives a real `StreamableHttpTransport` client against one era-target process under both v1 and v2 negotiation. v2 arm: Logging via the `_meta` mechanism returns the probed level from `LOG_SOURCE_REQUEST_META` (real field assertion, not a stub); Sampling and Roots both complete through the MRTR `InputRequiredResult` loop with `status: completed` (real round trip through `on_sampling`/`on_roots` host handlers). v1 mechanism proven separately by `v1_sampling_and_roots_complete_via_server_to_client_requests`, a DuplexTransport + `Server::run()` control with its own two-tool server — both complete with real payload fields (`model`, `rootUri`). `docs/v1-sunset-policy.md` states the 12-month advisory window (line 146-165), confirmed present. |

**Score:** 3/3 truths verified (all by live re-execution, not SUMMARY narrative)

### Anti-suppression audit (required by phase brief)

Scanned `scripts/run-conformance-suite.sh`, `scripts/run-era-matrix.sh`, `.github/workflows/ci.yml`,
and `tests/ci_conformance_gate_wiring.rs` for suppression mechanisms, matching **code lines**, not
comments/prose (the brief flagged that a prose-only false positive already occurred twice in this
phase):

| Pattern | Result |
|---------|--------|
| `--expected-failures` as an actual CLI argument | Not present. Only occurrences are prose (rationale comments) and the wiring test's own `FORBIDDEN_FLAGS` constant that asserts its absence. |
| `continue-on-error:` in `ci.yml` | Zero occurrences (`grep -n "continue-on-error" .github/workflows/ci.yml` → empty). |
| `\|\| true` masking a script's exit status | Zero occurrences reaching a conformance/era-matrix command (confirmed by `tests/ci_conformance_gate_wiring.rs::no_status_masking_reaches_an_era_matrix_command`, passing). |
| Known-fail allowlist of any shape | Not present. `ZERO_CHECK_SCORED_SCENARIOS=()` is empty (the strongest state per README §7) and is enforced by **bidirectional** set equality against what the suite actually reports — adding an entry to hide a real failure is structurally impossible (a failing scenario is by definition not zero-check). |
| Elevated workflow permissions | `grep -n "^permissions:" .github/workflows/ci.yml` → empty. |

No suppression mechanism found anywhere in the gate.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `conformance/{package.json,package-lock.json,.npmrc}` | Pinned, exact-version, integrity-hashed suite install | ✓ VERIFIED | Present; pin confirmed `0.2.0-alpha.11` (installed live and version-verified during re-execution). |
| `examples/s54_v2_dual_conformance.rs` | Dual-version conformance target, one process, both eras | ✓ VERIFIED | 1573 lines, substantive (not a stub). `cargo build --example s54_v2_dual_conformance --features full` succeeds. Registered in root `Cargo.toml:767-770` with `required-features = ["streamable-http", "testing"]`. |
| `scripts/run-conformance-suite.sh` | CONF-01 driver: one process, both requirement sets, MRTR + floor + zero-check gates | ✓ VERIFIED + executed live, passed |
| `scripts/run-era-matrix.sh` | CONF-02/CONF-03 driver: dev-dep-free build fences + nonzero-test-count guards | ✓ VERIFIED + executed live, passed |
| `tests/ci_conformance_gate_wiring.rs` | Bijective structural proof pinning script commands + `gate` wiring as data | ✓ VERIFIED — `cargo test --test ci_conformance_gate_wiring` → 16 passed, 0 failed |
| `.github/workflows/ci.yml` `conformance-suite` + `era-matrix` jobs | Two new CI jobs invoking the scripts above | ✓ VERIFIED — both present, correctly configured (Node 22 + npm cache for conformance-suite; timeouts on both) |
| `crates/pmcp-team-servers/src/conformance/{era_diff,era_observations,era_probe,era_target}.rs` | Ported Phase-117 era substrate | ✓ VERIFIED — present, compiles under both build fences |
| `crates/pmcp-team-servers/tests/{conformance,era_matrix,era_baseline}.rs` | v1 fixture regression guard, era comparison, baseline schema gate | ✓ VERIFIED — all executed live, all pass |
| `crates/pmcp-team-servers/baselines/era-deltas.yaml` | Checked-in expected-difference baseline | ✓ VERIFIED — present, schema-gated by `era_baseline.rs` (10/10 passing) |
| `docs/v1-sunset-policy.md` | 12-month advisory window, reconciled with removal condition | ✓ VERIFIED — present, contains the stated window |
| `.planning/phases/118.../118-CONFORMANCE-GAPS.md` | D-21 declared non-conformance, source-cited | ✓ VERIFIED — spot-checked G-1 (`src/types/content.rs:78-91`), G-4 (`src/server/mod.rs` catch-all `Complete(_) => Ok(json!({}))`), G-5 (`src/server/streamable_http_server.rs:2951` matches exactly `Subscribe`/`Unsubscribe`), G-7 (`grep -rn supportedVersions src/` → 0 hits) — all four citations confirmed accurate against current source |
| `Makefile` `test-conformance` / `test-era-matrix` targets | Local spelling of the CI commands | ✓ VERIFIED — present, invoke the same scripts |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `.github/workflows/ci.yml` `gate.needs` | `conformance-suite`, `era-matrix` jobs | job dependency array | ✓ WIRED | `needs: [test, quality-gate, purity-check, pmcp-agent-targets, wasm32-purity, v1-severance, conformance-suite, era-matrix]` — both present |
| `gate` job `env:` | `needs.conformance-suite.result`, `needs.era-matrix.result` | `CONFORMANCE_RESULT` / `ERA_MATRIX_RESULT` bindings | ✓ WIRED | Both bound; confirmed by `every_awaited_job_is_bound_read_and_named` (passing) |
| `gate` job conditional | `CONFORMANCE_RESULT` / `ERA_MATRIX_RESULT` | `if [[ "$X" != "success" ]] ... exit 1` chain | ✓ WIRED | Both clauses present in the `if` chain (verified by direct read + passing wiring test) |
| `gate` job failure echo | `CONFORMANCE_RESULT` / `ERA_MATRIX_RESULT` | `job=$VAR` pairs in the failure message | ✓ WIRED | Both present |
| `conformance-suite` CI job | `scripts/run-conformance-suite.sh` | `run:` step | ✓ WIRED | Sole step invoking the script (`each_job_invokes_exactly_one_driver_script`, passing) |
| `era-matrix` CI job | `scripts/run-era-matrix.sh` | `run:` step | ✓ WIRED | Same pattern, passing |
| `scripts/run-conformance-suite.sh` | official `conformance` CLI (npm) | `./"$SUITE_BIN" server --url ... --requirements <rev>` | ✓ WIRED + FLOWING | Confirmed by live execution — real HTTP requests against the real built binary, real `checks.json` produced on disk, real pass/fail counts read back |
| `scripts/run-era-matrix.sh` | `cargo test -p pmcp-team-servers` | per-target flags as data (`MATRIX_TESTS`) | ✓ WIRED + FLOWING | Confirmed by live execution — non-vacuous test counts for all 3 targets |
| `era_matrix.rs` v2 arm | live era-target server | `StreamableHttpTransport` + `with_protocol_version(2026-07-28)` | ✓ WIRED + FLOWING | `supports_negotiated_protocol_version` asserted true before use (a self-defending tripwire against the transport silently degrading to v1); tool-call payload fields (`level`, `source`, `status`) asserted against real values, not just HTTP status codes |

### Data-Flow Trace (Level 4)

Not performed via static grep tracing — superseded by **direct live re-execution** of both driver
scripts in this verification pass, which is strictly stronger evidence: the actual official suite
process, the actual dual-version server binary, and the actual `pmcp-team-servers` test binary were
all run to completion and their real output (`checks.json` counts, `running N tests` lines, tool-call
JSON payloads) was read back and matched against what the phase claims.

### Probe Execution

| Probe | Command | Result | Status |
|-------|---------|--------|--------|
| Official conformance suite (CONF-01) | `PATH=<node22>:$PATH PMCP_REQUEST_STATE_KEY=<random> bash scripts/run-conformance-suite.sh` | `2025-11-25`: 66 checks (floor 66), 51 passed/15 failed, suite exit 1 (captured, not verdict). `2026-07-28`: 178 checks (floor 178), 124 passed/54 failed, suite exit 1 (captured). MRTR surface: 14 scenarios, 36 checks, 0 failures. Zero-check gate: exact match. Script's own exit: 0 — `CONF-01 gates PASSED.` | ✓ PASS |
| Era matrix (CONF-02/CONF-03) | `bash scripts/run-era-matrix.sh` | Both build fences OK. `conformance`: 10 passed/0 failed/1 ignored. `era_matrix --features http`: 4 passed/0 failed. `era_baseline --features http`: 10 passed/0 failed. Script exit: 0. | ✓ PASS |
| CI gate wiring bijection | `cargo test --test ci_conformance_gate_wiring` | 16 passed, 0 failed | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|-------------|-----------------|--------------|--------|----------|
| CONF-01 | 118-01, 118-02, 118-04, 118-05, 118-08, 118-09 | Official suite pinned + runs in CI against dual-version example over real HTTP | ✓ SATISFIED | Live re-execution above; CI wiring confirmed blocking |
| CONF-02 | 118-03, 118-06, 118-07, 118-08, 118-09, 118-10 | Phase-109 harness gains v2 fixtures, v1 stays green, dev-dep-free build | ✓ SATISFIED | Live re-execution above |
| CONF-03 | 118-03, 118-06, 118-07, 118-08, 118-09 | Deprecated Roots/Sampling/Logging functional under v2 negotiation | ✓ SATISFIED | `deprecated_capabilities_complete_under_both_eras` + v1 mechanism control, both passing live |

No orphaned requirements: all IDs mapped to Phase 118 in `REQUIREMENTS.md`'s traceability table
appear in at least one plan's `requirements:` frontmatter, and every plan's declared requirement
appears in `REQUIREMENTS.md`.

**Documentation-only inconsistency (informational, non-blocking):** `.planning/REQUIREMENTS.md`
still shows `CONF-01`/`CONF-02`/`CONF-03` as `[ ]` unchecked (lines 924-926) and the traceability
table (lines 1039-1041) still reads "Pending", even though `.planning/ROADMAP.md:2225` marks
**Phase 118 complete** (`completed 2026-08-10`) and all 10 plans are checked `[x]`. This is a stale
bookkeeping artifact, not a functional gap — every requirement's actual claim was independently
re-verified against running code above. Recommend updating REQUIREMENTS.md checkboxes/traceability
to "Complete" in the same commit that closes this phase.

### Anti-Patterns Found

None. Scanned all phase-touched files (`scripts/run-conformance-suite.sh`, `scripts/run-era-matrix.sh`,
`.github/workflows/ci.yml`, `tests/ci_conformance_gate_wiring.rs`, `examples/s54_v2_dual_conformance.rs`,
`crates/pmcp-team-servers/src/conformance/*.rs`, `crates/pmcp-team-servers/tests/{conformance,era_matrix,era_baseline}.rs`,
`crates/pmcp-team-servers/baselines/era-deltas.yaml`, `conformance/README.md`) for `TBD`/`FIXME`/`XXX`/
`TODO`/`HACK`/`PLACEHOLDER` (case-sensitive) and case-insensitive `placeholder|coming soon|will be
here|not yet implemented`. Zero debt markers. The two "placeholder" hits are legitimate doc-comment
usages of the word (an elicitation default value description, and a capture-var rename concept), not
stub markers.

### Human Verification Required

None. All observable truths were independently confirmed by direct re-execution of the actual gate
scripts against the actual live infrastructure (suite process, dual-version server, test binaries).
No `<human-check>` blocks were found in any of the 10 plans.

### Deferred Items (known, pre-declared — reported, not scored as gaps)

| # | Item | Why deferred | Evidence |
|---|------|--------------|----------|
| 1 | Live CI check-run observation ("the gate actually goes red when a job fails") | Can only be confirmed on a real PR run after merge; this repo state is pre-merge on branch `phase-118-conformance` | Wiring proved offline instead, by the 16 bijection tests in `tests/ci_conformance_gate_wiring.rs` (all passing, independently re-run in this verification) |
| 2 | T-118-51 artifact inspection (results uploaded on green) | Needs a real CI artifact upload to inspect | `if: always()` + `actions/upload-artifact@v7` confirmed present and correctly conditioned in `ci.yml`; the actual uploaded artifact contents cannot be inspected pre-merge |
| 3 | Plan acceptance criterion `test -f justfile` (must fail) | Unsatisfiable — a tracked `justfile` predates Phase 118 by many releases (`git log` shows it added in `dec40935`, long before this phase) | Confirmed: `justfile` exists and is tracked; intent (a working `just`-based script surface) is satisfied, the criterion itself is a plan-template defect, not a phase gap |

### Gaps Summary

None. All three ROADMAP success criteria (CONF-01, CONF-02, CONF-03) were independently verified by
live re-execution of the actual CI-gate scripts against real infrastructure, not by trusting
SUMMARY.md narrative. The D-21 declared non-conformance (full suite not exiting 0 due to nine cited
SDK gaps) is a measured, honestly-stated property of the SDK that this phase's plans explicitly chose
not to suppress — re-execution reproduced the exact same numbers the gaps document claims, which is
itself strong evidence the declaration is accurate rather than aspirational. No suppression mechanism
(`--expected-failures`, allowlist, `continue-on-error`, `|| true`, elevated permissions) was found
anywhere in the gate. Both new CI jobs are genuinely wired into the blocking `gate` aggregate in all
four required places.

---

_Verified: 2026-08-10T02:35:54Z_
_Verifier: Claude (gsd-verifier)_
