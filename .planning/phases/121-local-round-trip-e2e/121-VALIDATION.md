---
phase: 121
slug: local-round-trip-e2e
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-23
---

# Phase 121 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Seeded from `121-RESEARCH.md` § Validation Architecture. Task IDs are filled in by the planner;
> rows below are keyed by test-function name until then.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` / `#[tokio::test]` (libtest); `tokio` 1 with `macros`, `rt-multi-thread`, `time` |
| **Config file** | none — cargo-native; targets declared implicitly by `crates/pmcp-openapi-server/tests/*.rs` |
| **Quick run command** | `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` |
| **Full suite command** | `cargo test -p pmcp-openapi-server -- --test-threads=1` |
| **Estimated runtime** | ~3-8 seconds. Measured by the planner on 2026-08-23: `cargo test -p pmcp-openapi-server -- --test-threads=1` → **32 passed, 2 ignored, 8 suites, 2.76s, exit 0**; `--test parity_replay` alone → `3 passed; 1 ignored`, 1.11s. |

**Whole-crate baseline (measured 2026-08-23, before any Phase 121 change):** `cargo test -p
pmcp-openapi-server -- --test-threads=1` → **32 passed, 2 ignored, exit 0**. The crate is green
today, so chaining `test-openapi-server` into `test-all` cannot turn `make quality-gate` red on a
pre-existing failure. Executors must re-measure and STOP if their baseline disagrees.

**Gate command (does not exist yet — Wave 0 blocker):** `make test-openapi-server`, chained into
`make test-all`. Until that exists, `make quality-gate` is green on a phase whose entire deliverable
it never executes (RESEARCH CF-2).

> ⚠ **Selector discipline (repo-specific; bit 7× in Phase 114).** Under `cargo nextest`,
> `-E 'test(/foo/)'` silently selects **zero** tests and exits 0. Every `<verify>` block in this
> phase must use plain `cargo test --test <name>`, which fails loudly on an unknown target.
> No `nextest -E 'test(...)'` selector may appear in any plan.

---

## Sampling Rate

- **After every task commit:** `cargo test -p pmcp-openapi-server --test <target> -- --test-threads=1`
- **After every plan wave:** `cargo test -p pmcp-openapi-server -- --test-threads=1` **plus**
  `make pmcp-package-gate` (the `pmcp-package` half *is* genuinely gated — RESEARCH CF-1 — so a
  slot-API regression surfaces there)
- **Before `/gsd-verify-work`:** `make quality-gate` green — **meaningful only once
  `test-openapi-server` is chained into `test-all`**
- **Max feedback latency:** ~5 seconds

---

## Per-Task Verification Map

> **Wave numbering, reconciled with the plans (2026-08-23).** The rows below were seeded with
> RESEARCH's `Wave 0 / 1 / 2` labels. The written plans use GSD wave numbers starting at 1, so
> the mapping is: RESEARCH Wave 0 = plan `121-01` (GSD wave 1), RESEARCH Wave 1 = plan `121-02`
> (GSD wave 2), RESEARCH Wave 2 = plan `121-03` (GSD wave 3). The **Wave** column below now
> carries the GSD wave number. The three plans are strictly sequential — 121-02 and 121-03 both
> modify `roundtrip_e2e.rs`, and 121-02 consumes helpers 121-01 creates — so no two run in
> parallel.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 121-01/T1 | 121-01 | 1 | PKG-04 (D-01) | T-121-01-SC | N/A | build | `cargo test -p pmcp-openapi-server --no-run` | ❌ W0 | ⬜ pending |
| 121-01/T1 | 121-01 | 1 | PKG-04 (D-03) | T-121-01-02 | Dev-dep pin is caret `"0.2"` in `[dev-dependencies]` | unit | `cargo test -p pmcp-openapi-server --test pmcp_package_pin -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 121-01/T2 | 121-01 | 1 | PKG-04 (CF-2 gate, D-13) | T-121-01-01 | Target fails when the summed test count is 0 | build | `make test-openapi-server` | ❌ W0 | ⬜ pending |
| 121-01/T3 | 121-01 | 1 | PKG-04 (D-02 lift) | T-121-01-03 | N/A | integration | `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` → must remain `3 passed; 1 ignored` | ✅ green | ⬜ pending |
| 121-02/T1 | 121-02 | 2 | PKG-04 / SC1 | T-121-02-05 | Two OCI layouts + temp dirs asserted distinct; differing endpoint/credential/auth; fully offline, two `wiremock` instances | integration | `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 121-02/T1 | 121-02 | 2 | PKG-04 / SC3a | T-121-02-06 | B's served `(name, inputSchema)` set equals A's; both non-empty and containing the 4 known names | integration | same target, `roundtrip_tool_surface_parity` (TRACER) | ❌ W0 | ⬜ pending |
| 121-02/T2 | 121-02 | 2 | PKG-04 / SC2a | T-121-02-02 | `required_slots` set-equals the hardcoded 3-slot literal | integration | same target, `roundtrip_required_slots_match_expected_literal` | ❌ W0 | ⬜ pending |
| 121-02/T2 | 121-02 | 2 | PKG-04 / SC2b | T-121-02-02 | `detect_deviation` reports B's endpoint drift; returns `None` for the credential | integration | same target, `roundtrip_endpoint_drift_is_reported` | ❌ W0 | ⬜ pending |
| 121-02/T3 | 121-02 | 2 | PKG-04 / SC3b | T-121-02-03 | `london-tube-scenarios.yaml` replays green in B, per-step gated, `steps_total > 0` | integration | same target, `roundtrip_scenarios_replay_green_in_env_b` | ❌ W0 | ⬜ pending |
| 121-03/T1 | 121-03 | 3 | PKG-04 / SC4-red | T-121-03-02 | Degraded B (tool removed) → comparison returns `Err` naming that tool | integration | same target, `degraded_env_b_missing_tool_is_reported` | ❌ W0 | ⬜ pending |
| 121-03/T1 | 121-03 | 3 | PKG-04 / SC4-red | T-121-03-04 | Degraded B (named slot unfilled) → assembly fails naming that slot | integration | same target, `degraded_env_b_unfilled_slot_is_reported` | ❌ W0 | ⬜ pending |
| 121-03/T2 | 121-03 | 3 | PKG-04 / SC4-green | T-121-03-01 | No assertion on manifest field names / layer ordering / digest values, with a nonzero-lines-scanned floor | integration | same target, `roundtrip_e2e_asserts_nothing_about_manifest_shape` | ❌ W0 | ⬜ pending |
| 121-03/T3 | 121-03 | 3 | PKG-04 (doc correction) | — | `detect_deviation`'s rustdoc matches its `classify`-driven behaviour | build | `make pmcp-package-gate` | ✅ green | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

## Mutation / teeth proofs (each task's non-vacuity evidence)

A passing test proves nothing about a test's sensitivity. Every task in this phase carries at
least one required proof that the assertion fails when the property it measures is broken. These
are acceptance criteria, not suggestions — a task whose proof was skipped is not done.

| Plan/Task | Proof | Expected |
|-----------|-------|----------|
| 121-01/T1 | Drift the dev-dep pin to `0.3` | pin tripwire FAILS; revert → passes |
| 121-01/T2 | Pipe a canned output with no `test result:` line through the target's awk program | prints `0`; with a `5 passed` line → prints `5` |
| 121-01/T3 | (regression guard, not a mutation) | `parity_replay` must remain exactly `3 passed; 1 ignored` |
| 121-02/T1 | Truncate B's snapshot by one entry | parity test FAILS naming the dropped tool |
| 121-02/T1 | Point B at a bogus bind address | FAILS at the `TestStatus::Passed` assertion, not green on two empty snapshots |
| 121-02/T2 | Alter one character of the expected auth-mode slot name | set-equality FAILS; revert → passes |
| 121-02/T2 | Pass A's endpoint as the proposed value | `detect_deviation` → `None`, test FAILS |
| 121-02/T3 | Mount B's wiremock under A's credential | per-step gate FAILS with named failing steps (the CF-6 mode) |
| 121-02/T3 | Point the scenario loader at a zero-step scenario in a `TempDir` | FAILS at the `steps_total` floor |
| 121-03/T1 | Make the tool-removal degradation a no-op | negative test FAILS because the comparison returned `Ok` |
| 121-03/T1 | Leave the endpoint variable SET | negative test FAILS because assembly succeeded |
| 121-03/T2 | Plant one executable assertion line containing an algorithm-prefixed content-hash literal | structural guard FAILS naming the token; remove → passes |
| 121-03/T2 | Make the line filter match nothing | FAILS at the scanned-line floor with the "not reaching the file" message |

After every proof: revert, re-run, and confirm `git status --porcelain` is clean of proof
artifacts. `crates/pmcp-openapi-server/tests/fixtures/` must be untouched throughout — degrade
copies inside a `TempDir`, never the checked-in fixture.

---

## Wave 0 Requirements

- [ ] `crates/pmcp-openapi-server/Cargo.toml` — add `pmcp-package = { version = "0.2", path = "../pmcp-package" }` and `toml = "0.8"` to `[dev-dependencies]`; prove resolution with `cargo test -p pmcp-openapi-server --no-run`
- [ ] `crates/pmcp-openapi-server/tests/common/mod.rs` — the D-02 helper lift, with `#![allow(dead_code)]`, per-binary-mutex reasoning, and `mount_london_tube` parameterized by `app_key` (RESEARCH CF-6)
- [ ] `Makefile` — `test-openapi-server` target chained into `test-all` (RESEARCH CF-2) **with a nonzero-test-count guard**
- [ ] `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` — D-03 tripwire, reading the `[dev-dependencies]` table (not `[dependencies]`)
- [ ] `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` — the E2E (plan 121-02), the D-08 negatives and the D-09 structural guard (plan 121-03)
- [x] ~~`.planning/ROADMAP.md` (2 sites) + `.planning/REQUIREMENTS.md:26` — finish D-05's correction~~ — **ALREADY DONE**, committed as `91dd3978` by the orchestrator before planning. RESEARCH CF-5 / OQ-4 are closed. No task exists for this; do not re-do it.
- [ ] `crates/pmcp-package/src/slot/deviation.rs` — the stale rustdoc (CONTEXT roadmap correction #2), plan 121-03 Task 3. Verified by `make pmcp-package-gate`, not by a docs build — editing that crate pulls in its format check, `clippy -D warnings` and full test suite.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| — | — | — | — |

*All phase behaviors have automated verification.* This is a test-only phase; its deliverable **is**
the automated verification. The one judgement call that resists automation — whether the D-09
structural guard's deny-list is the *right* deny-list — is mitigated by a nonzero-lines-scanned
floor rather than by manual review.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] No `nextest -E 'test(...)'` selectors in any `<verify>` block (repo-specific false-green)
- [ ] Every new test asserts a **nonzero** count of the thing it measures (repo has shipped
      exit-0-measuring-nothing shapes three times: CR-01, the RTK-truncated gate run, and
      `list_tools()`'s `unwrap_or_default()` — RESEARCH CF-3)
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
