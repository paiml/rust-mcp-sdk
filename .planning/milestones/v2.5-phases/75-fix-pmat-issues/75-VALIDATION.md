---
phase: 75
slug: fix-pmat-issues
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-22
---

# Phase 75 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution. Derived
> from `75-RESEARCH.md` § Validation Architecture and locked decisions in
> `75-CONTEXT.md` (D-01..D-09).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `cargo test` (workspace), `proptest` 1.6, `trybuild` 1.0, `insta` 1.43 |
| **Config files** | `Cargo.toml` (workspace root), `pmcp-macros/Cargo.toml` (test deps) |
| **Quick run command** | `cargo test --workspace --lib -- --test-threads=1` |
| **Full suite command** | `make quality-gate` then `cargo test --workspace --all-features --verbose -- --test-threads=1` |
| **PMAT measurement** | `pmat quality-gate --fail-on-violation --checks complexity` (must show monotonic decrease per wave) |
| **Estimated full-suite runtime** | ~600s on cold cache, ~120s warm |

---

## Sampling Rate

- **After every task commit:** `cargo test --workspace --lib` + `cargo clippy --all-targets --all-features -- -D warnings` (lighter than `make quality-gate`, fast feedback).
- **After every plan wave:** Full `make quality-gate` AND `pmat quality-gate --fail-on-violation --checks complexity`. Record post-wave violation count in the wave-merge commit message: `pmat-complexity: NN (was MM)`.
- **Before `/gsd-verify-work`:** Full `pmat quality-gate --fail-on-violation` (all checks) exits 0; `cargo test --workspace --all-features` green.
- **Max feedback latency:** ~30s per-task quick run; ~10min per-wave full run.

---

## Per-Wave Verification Map

> Phase 75 has no REQ-IDs (quality-debt remediation, not feature work). Validation is per-wave functional regression + per-commit complexity reduction.

| Wave | Plan | Behavior gate | Test Type | Automated Command | File Exists | Status |
|------|------|--------------|-----------|-------------------|-------------|--------|
| 0 | 75-00 | Empirical PMAT path-filter test | spike | `pmat quality-gate --include 'src/**' --checks complexity` (verify per `pmat --help`) | ❌ W0 spike | ⬜ pending |
| 0 | 75-00 | `pmcp-macros` insta snapshot baseline | snapshot | `cargo test -p pmcp-macros --test expansion_snapshots` | ❌ W0 creates | ⬜ pending |
| 0 | 75-00 | `pmcp-code-mode` semantic regression baseline | unit | `cargo test -p pmcp-code-mode evaluator::semantic_regression` | ❌ W0 creates | ⬜ pending |
| 1a | 75-01 | streamable_http_server transport unchanged | unit + integration | `cargo test --test streamable_http_server_tests --test streamable_http_unit_tests` | ✅ existing | ⬜ pending |
| 1a | 75-01 | streamable_http_server property invariants | property | `cargo test --test streamable_http_properties` | ✅ existing | ⬜ pending |
| 1b | 75-01 | pmcp-macros expansion stable (snapshots) | snapshot | `cargo test -p pmcp-macros` | ✅ after W0 baseline | ⬜ pending |
| 1b | 75-01 | pmcp-macros downstream still compiles | compile | `cargo test --workspace --all-features` | ✅ implicit | ⬜ pending |
| 1b | 75-01 | trybuild compile-fail tests still pass | compile-fail | `cargo test -p pmcp-macros --test trybuild_tests` | ❓ planner verifies | ⬜ pending |
| 2a | 75-02 | pentest behavior unchanged | integration | `cargo test -p cargo-pmcp pentest::` + `cargo run -p cargo-pmcp -- pentest --dry-run` (if exists) | ❓ planner verifies | ⬜ pending |
| 2b | 75-02 | deployment behavior unchanged | unit | `cargo test -p cargo-pmcp deployment::` | ✅ assumed | ⬜ pending |
| 3 | 75-03 | code-mode evaluator semantic-equivalent | unit + semantic | `cargo test -p pmcp-code-mode` (semantic baseline from Wave 0) | ✅ after W0 | ⬜ pending |
| 4 | 75-04 | scattered hotspots refactor regressions | unit | `cargo test --workspace -- --test-threads=1` | ✅ existing | ⬜ pending |
| 5 | 75-05 | CI gate fails on complexity regression | integration | Open dummy PR with deliberately complex function; verify `gate` job fails | manual one-time | ⬜ pending |
| 5 | 75-05 | README badge flips to passing | manual | Wait for next `quality-badges.yml` run on `main`; verify badge shows `passing` | manual | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

These MUST land before Wave 1 starts — they are the regression-detection baseline for the whole phase.

- [ ] **PMAT path-filter empirical test** — Write a Wave 0 spike task that runs `pmat quality-gate --fail-on-violation --checks complexity --include 'src/**'` (and any other documented path-filter flags from `pmat quality-gate --help`). Record whether examples/ violations get excluded. Result determines D-09 implementation path: (a) path filter works → CI gate uses `--include`; (b) path filter doesn't work → bulk `#[allow(clippy::cognitive_complexity)]` on examples/ functions with `// Why: illustrative demo code`.
- [ ] **`pmcp-macros` insta snapshot baseline** — Add `pmcp-macros/tests/expansion_snapshots.rs` with `insta::assert_snapshot!` over the macro expansion of representative MCP tool/server/prompt definitions (use `proc-macro2` token-stream-to-string or `prettyplease` if available). Run BEFORE Wave 1b refactor. The snapshots become the regression contract.
- [ ] **`pmcp-code-mode` semantic baseline** — Verify `crates/pmcp-code-mode/tests/` covers `evaluate_with_scope` and `evaluate_array_method_with_scope` (the highest-complexity functions from research) at semantic-equivalence level. If coverage is shallow, add semantic regression tests with representative input programs and expected output values.
- [ ] **PMAT version pin in CI** — Update `.github/workflows/ci.yml` and `.github/workflows/quality-badges.yml` to install a pinned PMAT version (e.g. `cargo install pmat --version 3.15.0 --locked`) so the gate semantics don't drift between local and CI as PMAT releases.

*If Wave 0 is skipped, Phase 75 has NO regression detection for the macro and code-mode refactors. Do not skip.*

---

## Manual-Only Verifications

| Behavior | Why Manual | Test Instructions |
|----------|------------|-------------------|
| README badge flip | Async — driven by scheduled `quality-badges.yml` cron at 06:00 UTC + on push to main | After Wave 5 lands on main, wait for next workflow run; check `<!-- QUALITY BADGES START -->` block in README shows `Quality Gate-passing-brightgreen`. |
| CI gate blocks regression | Requires opening a deliberately-bad PR | After Wave 5 lands, open a test PR adding a trivially-too-complex function (cognitive complexity 30+ via deeply nested matches). Verify the `gate` aggregate job fails on PMAT step. Close PR without merging. |
| `pentest` dry-run output | Network-touching command; not always safe in unit tests | Before/after each Wave 2a commit: `cargo run -p cargo-pmcp -- pentest --dry-run --target localhost:1234` (or whatever the existing example invocation is). Diff stdout. |

---

## Validation Sign-Off

- [ ] All waves have automated verify (or W0 dependency listed above)
- [ ] Sampling continuity: no 3 consecutive task commits without a `cargo test --workspace --lib` run
- [ ] Wave 0 covers all `❌ W0` references in the verification map
- [ ] No watch-mode flags (`--watch`, `--no-fail-fast`)
- [ ] PMAT complexity count recorded in every wave-merge commit message
- [ ] PMAT version pinned in CI (no floating `cargo install pmat`)
- [ ] `nyquist_compliant: true` set in frontmatter once Wave 0 lands

**Approval:** pending
