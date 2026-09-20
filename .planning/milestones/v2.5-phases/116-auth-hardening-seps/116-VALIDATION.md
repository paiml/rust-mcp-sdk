---
phase: 116
slug: auth-hardening-seps
status: approved
nyquist_compliant: true
wave_0_complete: false
created: 2026-08-02
revised: 2026-08-03
---

# Phase 116 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
>
> **Revised 2026-08-03** after cross-AI review (`116-REVIEWS.md`). Changes: plan 116-16 added (the
> gated `FileCredentialStore`, split out of 116-05 when D-116-R1/R2 grew it past budget); the
> `oauth_credential_file` binary added; a second fuzz target added in 116-08; and the automated
> command form hardened — see "Command form" below.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test / cargo nextest (Rust) |
| **Config file** | Cargo.toml (workspace root) |
| **Quick run command** | `cargo test --features full,oauth --lib` |
| **Full suite command** | `make quality-gate` PLUS `cargo test --features full,oauth` (oauth is NOT in the `full` feature — the gate alone proves nothing for this phase) |
| **Estimated runtime** | ~130 seconds |

---

## Command form (MANDATORY — every `<automated>` block in this phase uses it)

```
mkdir -p target/116-verify && set -o pipefail && \
  cargo nextest run --features full,oauth -E 'binary(NAME)' 2>&1 | tee target/116-verify/NAME.log && \
  grep -qE 'Summary \[.*\] [1-9][0-9]* tests? run' target/116-verify/NAME.log
```

Three rules, each closing a measured failure mode:

1. **`binary(...)`, never a bare `test(...)`.** A `test(...)` selector can silently select ZERO tests
   and exit 0 — this bit Phase 114 seven times — and it also mis-selects on substrings. Measured in
   this repo: an unbounded substring selector on the word "auth" skips 4 of
   `cargo-pmcp/tests/auth_integration.rs`'s 7 tests (including `logout_no_args_errors_via_cli`, the
   load-bearing `auth logout` semantic) while sweeping in `workbook_explain.rs` and
   `deploy_post_deploy_flags.rs`, so a non-zero count passes with the regression-critical tests never
   run. A substring predicate is permitted ONLY in the compound form `binary(X) and test(Y)`, where
   `binary(X)` bounds the selection to one target. Every `-E` expression in this phase therefore
   contains `binary(`.
2. **The count is PARSED, not tailed.** `cargo nextest` with a filter matching nothing exits 0
   having run nothing, so a green exit code proves nothing on its own. The `Summary [...] N tests
   run` grep asserts N is non-zero.
3. **`set -o pipefail` and `&&`, never `;` or a bare `| tail`.** Without `pipefail` a pipeline
   reports the LAST command's status, so `cargo … | tail` reports success after `cargo` failed.

---

## Sampling Rate

- **After every task commit:** Run `cargo test --features full,oauth --lib`
- **After every plan wave:** Run `make quality-gate` + `cargo test --features full,oauth`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 180 seconds

---

## Per-Task Verification Map

One row per plan × test binary (task-level `<automated>` blocks in each PLAN.md carry the exact
commands, all in the form above).

| Plan | Wave | Requirement | Test Binaries | Status |
|------|------|-------------|---------------|--------|
| 116-01 | 1 | AUTH-01/02/03 | `v2_bounded_reads_tripwire` (observation only, reverted); plus `make comply` and a YAML parse over both contract files | ⬜ pending |
| 116-02 | 2 | AUTH-01 | `binary(oauth_iss_validation)` (also ungated under `--features full`), `binary(pmcp) and (test(iss_mismatch) + test(state_mismatch) + test(reauth_required))`, `--doc error` | ⬜ pending |
| 116-03 | 2 | AUTH-02 | `binary(pmcp) and test(application_type)` | ⬜ pending |
| 116-04 | 3 | AUTH-02/03 | `oauth_discovery_urls` (also ungated), `oauth_application_type` | ⬜ pending |
| 116-05 | 3 | AUTH-03 | `oauth_credential_store` (also ungated) + wasm32 CI fence | ⬜ pending |
| 116-06 | 4 | AUTH-01/03 | `binary(oauth_discovery_validation)`, `binary(pmcp) and (test(within_cap) + test(hardened_discovery_client))` | ⬜ pending |
| 116-07 | 5 | AUTH-03 | `oauth_provider_discovery` | ⬜ pending |
| 116-08 | 4 | AUTH-01/02/03 | fuzz targets `oauth_authorization_response` AND `oauth_credential_and_dcr` + `cargo run --example c11_oauth_iss_state_validation` (ALWAYS reqs; no nextest binary) | ⬜ pending |
| 116-09 | 5 | AUTH-01 | `oauth_iss_integration`, `oauth_state_csrf` | ⬜ pending |
| 116-10 | 6 | AUTH-02/03 | `binary(oauth_dcr_integration)` (count must exceed the baseline 5, asserted numerically), `binary(pmcp) and test(application_type_divergence)` | ⬜ pending |
| 116-11 | 7 | AUTH-03 | `oauth_store_wiring` | ⬜ pending |
| 116-12 | 8 | AUTH-03 | `oauth_refresh`, plus the five-binary regression sweep (`oauth_dcr_integration`, `oauth_iss_integration`, `oauth_state_csrf`, `oauth_store_wiring`, `oauth_refresh`) | ⬜ pending |
| 116-13 | 9 | AUTH-03 | `cargo-pmcp` workspace tests + `binary(auth_integration)` (and `binary(cargo_pmcp) and test(auth_cmd)` if inline unit tests land); `cargo check --workspace --locked` | ⬜ pending |
| 116-14 | 9 | AUTH-03 | `v2_bounded_reads_tripwire` (green under `full,oauth` AND under `full` alone) | ⬜ pending |
| 116-15 | 10 | AUTH-01/02/03 | all 14 binaries (closing full-sweep, every count parsed and non-zero) | ⬜ pending |
| 116-16 | 5 | AUTH-03 | `oauth_credential_file` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

**The fourteen binaries 116-15 sweeps:** `oauth_iss_validation`, `oauth_discovery_urls`,
`oauth_application_type`, `oauth_credential_store`, `oauth_credential_file`,
`oauth_discovery_validation`, `oauth_provider_discovery`, `oauth_state_csrf`,
`oauth_iss_integration`, `oauth_dcr_integration`, `oauth_store_wiring`, `oauth_refresh`,
`v2_bounded_reads_tripwire`, plus cargo-pmcp's `binary(auth_integration)` selection.

---

## Gate acceptance classes (116-15 Task 1)

Recorded here so the execution loop uses the same rule the booking plan does.

- **Class A — REQUIRED-GREEN (11 gates):** `make quality-gate`; `cargo nextest run --features
  full,oauth` with parsed non-zero per-binary counts; clippy with pedantic+nursery; `pmat
  quality-gate --checks complexity`; `cargo semver-checks --baseline-rev b2bf9157`;
  `make wasm-build`; `make check-todos`; `make test-examples` + the `c11` example; both fuzz
  campaigns; the refined dependency fence including `Cargo.lock`; `make comply` + contract-binding
  resolution. Any red STOPS the phase.
- **Class B — ACCEPTED BASELINE DELTA (1 gate):** `make doc-check` only. Acceptance is a two-part
  delta against the anchor recorded in `116-BASELINES.md`: (B1) total `^error` count ≤ anchor AND
  (B2) zero errors in any file this phase touched. `make quality-gate` does NOT chain `doc-check`
  (Makefile:673-694), which is what makes the two classes independent rather than contradictory.
  Either part failing means it is treated as Class A red.

Nothing else may enter Class B. A gate red for a NEW reason is Class A red.

---

## Wave 0 Requirements

None as a separate wave — TDD-shaped tasks in the plans land test + implementation together, so no
standalone stub-creation wave is required. Existing infrastructure covers all phase requirements,
with one standing check:

- [ ] Confirm `cargo test --features full,oauth` compiles and selects the oauth integration tests
  (measured pre-planning: `binary(oauth_dcr_integration)` selects 0 tests under `--features full`,
  5 under `--features full,oauth`) — re-verified in 116-01 baselines

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live Entra ID discovery-URL probe behavior | AUTH-03 (D-13/A3) | Requires network access to login.microsoftonline.com | Probe tenant discovery URL candidates in documented order; appended form must be attempted first |
| Cross-process credential-file locking under real OS process separation | AUTH-03 (116-16) | The in-repo tests use two `FileCredentialStore` instances rather than two OS processes | Run two `cargo pmcp auth login` invocations against different servers concurrently; both logins must survive |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (no separate Wave 0 needed — tests land with implementation)
- [x] No watch-mode flags
- [x] Every `<automated>` block uses `binary(...)` selectors and a PARSED non-zero count assertion
- [x] No `<automated>` block uses `;` as a command separator, and every pipeline is preceded by `set -o pipefail`
- [x] Feedback latency < 180s (two flagged exceptions: 116-08's TWO fuzz campaigns each run AT the
      180s ceiling — inherent to the CLAUDE.md ALWAYS-fuzz requirement, accepted as awareness-only)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-08-02 (plan-checker pass: 0 blockers).
**Revised 2026-08-03** for the cross-AI review replan: 16 plans, `oauth_credential_file` binary added,
second fuzz target added, command form hardened, gate acceptance classes recorded.
</content>
