---
phase: 76
plan: 05
subsystem: cargo-pmcp
tags: [cargo-pmcp, iam, fuzz, example, docs, release, wave-5, closeout]
requires: [76-01, 76-02, 76-03, 76-04]
provides:
  - "fuzz_iam_config libfuzzer target (T-76-03 mitigation)"
  - "cargo run -p cargo-pmcp --example deploy_with_iam"
  - "examples/fixtures/cost-coach.deploy.toml reference fixture"
  - "DEPLOYMENT.md ## IAM Declarations section"
  - "README.md Declarative IAM pointer"
  - "CHANGELOG.md (new file) — 0.10.0 entry"
  - "cargo-pmcp 0.10.0 version bump"
  - "narrow lib.rs exposure of deployment::{config,iam} + utils::config"
affects:
  - cargo-pmcp/fuzz/fuzz_targets/fuzz_iam_config.rs (created)
  - cargo-pmcp/fuzz/Cargo.toml (added [[bin]] entry)
  - cargo-pmcp/fuzz/corpus/fuzz_iam_config/ (created with 3 seeds + .gitignore)
  - cargo-pmcp/examples/deploy_with_iam.rs (created)
  - cargo-pmcp/examples/fixtures/cost-coach.deploy.toml (created)
  - cargo-pmcp/Cargo.toml (version bump 0.9.1 → 0.10.0)
  - cargo-pmcp/DEPLOYMENT.md (appended IAM section)
  - cargo-pmcp/README.md (added Declarative IAM bullet)
  - cargo-pmcp/CHANGELOG.md (created)
  - cargo-pmcp/src/lib.rs (narrow deployment + utils exposure)
tech-stack:
  added: ["libfuzzer-sys fuzz target for DeployConfig/IamConfig"]
  patterns:
    - "#[path] mounting to narrow lib-visible subset of a bin-only module tree"
    - "Keep-a-Changelog 1.1.0"
    - "Semver minor bump for additive features (CLAUDE.md §Version Bump Rules)"
key-files:
  created:
    - cargo-pmcp/fuzz/fuzz_targets/fuzz_iam_config.rs
    - cargo-pmcp/fuzz/corpus/fuzz_iam_config/seed_empty.toml
    - cargo-pmcp/fuzz/corpus/fuzz_iam_config/seed_cost_coach.toml
    - cargo-pmcp/fuzz/corpus/fuzz_iam_config/seed_wildcard.toml
    - cargo-pmcp/fuzz/corpus/fuzz_iam_config/.gitignore
    - cargo-pmcp/examples/deploy_with_iam.rs
    - cargo-pmcp/examples/fixtures/cost-coach.deploy.toml
    - cargo-pmcp/CHANGELOG.md
  modified:
    - cargo-pmcp/fuzz/Cargo.toml
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/DEPLOYMENT.md
    - cargo-pmcp/README.md
    - cargo-pmcp/src/lib.rs
decisions:
  - "Narrow the lib.rs exposure to `deployment::{config,iam}` + `utils::config` via #[path] mounts rather than declaring the full `deployment` module public. The full tree cross-depends on `crate::commands::*` (bin-only) via `deployment::targets::aws_lambda::{init,deploy}.rs`; a blanket `pub mod deployment;` in lib.rs would cascade into the entire CLI layer. The #[path] approach preserves bin/lib separation while giving the fuzz target and example the exact public API surface they need."
  - "Auto-discovered corpus files from `cargo fuzz run` are excluded via `.gitignore` (only `seed_*.toml` checked in). Prevents future fuzz-run pollution of commits."
  - "Contract-first check: SKIPPED — `../provable-contracts/contracts/cargo-pmcp` is not present adjacent to rust-mcp-sdk at execution time (2026-04-22). CLAUDE.md §Contract-First Development permits skip when the adjacent repo is not available; this preserves auditability."
metrics:
  duration: "~50 minutes"
  completed: "2026-04-22"
  tasks_completed: 3
  tasks_total: 3
  tests_passing: 779 (cargo test -p cargo-pmcp)
  example_exit_code: 0
  fuzz_smoke_executions: 170000
  fuzz_smoke_duration_seconds: 10
  quality_gate: PASSED
---

# Phase 76 Plan 05: Wave 5 Closeout (fuzz + example + docs + 0.10.0) Summary

Phase 76's final wave discharges the Toyota Way ALWAYS coverage mandate
(fuzz + example — Waves 1–4 already landed property + unit tests) and ships
the release metadata for `cargo-pmcp` 0.10.0. The authoritative gate —
`make quality-gate` — passes end-to-end.

## One-liner

Adds fuzz_iam_config libfuzzer target (T-76-03 mitigation) + `cargo run --example deploy_with_iam` end-to-end walk + DEPLOYMENT.md/CHANGELOG.md/README.md + 0.10.0 version bump; `make quality-gate` green.

## Tasks

| # | Task | Commit | Files | Key output |
|---|------|--------|-------|------------|
| 1 | Fuzz target + corpus seeds + fuzz/Cargo.toml registration | `b5e324d4` | 5 created, 2 modified | 170K runs in 10s smoke, zero panics |
| 2 | Runnable example + cost-coach fixture | `98f835f1` | 2 created, 3 modified (seeds) | `cargo run --example deploy_with_iam` exits 0 |
| 3 | Docs + version bump + CHANGELOG + `make quality-gate` | `3923834a` | 1 created, 4 modified | Quality gate PASSED |

## Verification evidence

```
$ cargo run -p cargo-pmcp --example deploy_with_iam
=== Phase 76 — Declare IAM in .pmcp/deploy.toml ===
...
--- 3. Validating ---
  Valid (no warnings).
--- 4. Rendered TypeScript addToRolePolicy block ---
  mcpFunction.addToRolePolicy(new iam.PolicyStatement({
    effect: iam.Effect.ALLOW,
    actions: ['dynamodb:GetItem', 'dynamodb:Query', 'dynamodb:Scan', 'dynamodb:BatchGetItem', 'dynamodb:PutItem', 'dynamodb:UpdateItem', 'dynamodb:DeleteItem', 'dynamodb:BatchWriteItem'],
    ...
  }));
--- 5. Demonstrating validator rejects wildcard Allow ---
  Validator correctly rejected the invalid config:
  [iam.statements][0]: Allow + actions=["*"] + resources=["*"] is a wildcard escalation footgun — refuse to deploy. Tighten actions and resources, or use [[iam.tables]] / [[iam.buckets]] sugar blocks.
=== Example complete ===
(exit 0)

$ cd cargo-pmcp/fuzz && cargo +nightly fuzz run fuzz_iam_config -- -max_total_time=10
Done 170942 runs in 11 second(s)   (zero panics)

$ cargo test -p cargo-pmcp
cargo test: 779 passed (8 suites, 7.08s)

$ make quality-gate
✓ Code formatting OK
✓ No lint issues
✓ All test suites passed (ALWAYS requirements met)
✅ ALL ALWAYS requirements validated!
✅ ALL TOYOTA WAY QUALITY CHECKS PASSED
🎯 ALWAYS Requirements Validated
(exit 0)
```

## Acceptance criteria (plan)

| Criterion | Status |
|-----------|--------|
| `fuzz_iam_config` libfuzzer target exists + compiles on nightly | PASS |
| 3 corpus seeds (empty, cost-coach, wildcard) | PASS |
| `fuzz/Cargo.toml` registers the [[bin]] entry | PASS |
| `cargo run -p cargo-pmcp --example deploy_with_iam` exits 0 + prints rendered addToRolePolicy + validator-rejection path | PASS |
| `cargo-pmcp/Cargo.toml` version 0.10.0 | PASS |
| DEPLOYMENT.md has `## IAM Declarations` section | PASS |
| README.md mentions IAM + links DEPLOYMENT.md | PASS |
| CHANGELOG.md has `[0.10.0]` entry covering PART-1 + PART-2 | PASS |
| `make quality-gate` passes end-to-end | PASS |
| `cargo test -p cargo-pmcp` green | PASS (779 tests) |
| `cargo build --examples -p cargo-pmcp` zero warnings | PASS |

## Deviations from plan

### Auto-fixed issues

**1. [Rule 3 — Blocking] Library target did not expose `deployment::{config,iam}`**
- **Found during:** Task 1 (fuzz build)
- **Issue:** `cargo +nightly fuzz build fuzz_iam_config` failed with `cannot find 'deployment' in 'cargo_pmcp'`. The lib target previously only exposed `loadtest` + `pentest` + a narrow test-support cache seam. A blanket `pub mod deployment;` would cascade into `crate::commands::*` (bin-only).
- **Fix:** Added a narrow `#[path]`-mounted subset: `pub mod deployment { pub mod config; pub mod iam; }` plus `pub mod utils { pub mod config; }`. This exposes exactly what the fuzz target and example need without pulling in `deployment::targets::aws_lambda::{init,deploy}.rs` (which reference `crate::commands::*`).
- **Files modified:** `cargo-pmcp/src/lib.rs`
- **Commit:** `b5e324d4`

**2. [Rule 1 — Bug] Plan's cost-coach fixture was missing required DeployConfig sections**
- **Found during:** Task 2 (running the example)
- **Issue:** Plan's fixture TOML contained only `[target]`, `[aws]`, `[server]` (minimal). The real `DeployConfig` schema requires `environment`, `auth`, `observability`, `target.version`, `server.memory_mb`, `server.timeout_seconds`. The example panicked at `toml::from_str::<DeployConfig>` with `missing field 'memory_mb'` → `missing field 'environment'`.
- **Fix:** Added the required fields with sensible defaults (`auth.enabled = false`, `observability.log_retention_days = 30`, etc.) to both the fixture file and the inline `INVALID_TOML` in the example. Also updated the three fuzz corpus seeds for consistency.
- **Files modified:** `cargo-pmcp/examples/fixtures/cost-coach.deploy.toml`, `cargo-pmcp/examples/deploy_with_iam.rs`, `cargo-pmcp/fuzz/corpus/fuzz_iam_config/seed_*.toml`
- **Commit:** `98f835f1`

**3. [Rule 2 — Missing hygiene] Auto-generated fuzz corpus pollution**
- **Found during:** Task 1 (first commit attempt)
- **Issue:** A fuzz smoke run during Task 1 verification auto-discovered 1,691 corpus inputs and wrote them to `cargo-pmcp/fuzz/corpus/fuzz_iam_config/`. The first `git add` pulled all of them into the commit (1,694 files changed).
- **Fix:** Reset the polluted commit (`git reset --soft HEAD~1`), deleted auto-generated files (kept only `seed_*.toml`), added a `.gitignore` that keeps only `seed_*.toml` files, then re-committed cleanly (7 files changed).
- **Files modified:** `cargo-pmcp/fuzz/corpus/fuzz_iam_config/.gitignore` (new)
- **Commit:** `b5e324d4`

**4. [Plan correction] `make quality-gate` flagged formatting on `deploy_with_iam.rs`**
- **Found during:** Task 3 (pre-gate fmt-check)
- **Issue:** `cargo fmt --all --check` flagged missing trailing commas in match arms.
- **Fix:** `cargo fmt --all` applied automatically; the resulting diff landed with Task 3.
- **Commit:** `3923834a`

### Contract-first check (CLAUDE.md §Contract-First Development)

**Contract-first check: SKIPPED (provable-contracts repo not adjacent to rust-mcp-sdk at execution time on 2026-04-22).**

The CLAUDE.md §Contract-First Development mandate requires updating the
contract YAML at `../provable-contracts/contracts/<crate>/`. The adjacent
`provable-contracts` repo does not exist at `../provable-contracts` or at
`/Users/guy/Development/mcp/sdk/provable-contracts` at execution time.
Documented per plan Task 3 Part E instructions as a compliant skip.

## Pre-existing clippy errors (deferred-items.md) — status

Wave 1's `deferred-items.md` flagged 20 pre-existing clippy errors in
`cargo-pmcp` (pentest/, deployment/metadata.rs, secrets/, commands/pentest.rs,
deployment/config.rs line 494 collapsible-if, etc.) when running
`cargo clippy -p cargo-pmcp --all-targets -- -D warnings`.

**Status at Wave 5:** `make quality-gate` PASSED. The project's authoritative
clippy command runs `cargo clippy --features "full" --lib --tests` on the
root `pmcp` workspace, not the stricter `cargo-pmcp --all-targets` form.
The deferred clippy errors remain in the codebase but are **not gated by
`make quality-gate`** — which CLAUDE.md declares is the sole authoritative
pre-commit/CI check. No `--no-verify` bypass was required.

**Recommendation for a follow-up phase (e.g., 76.1 gap closure or a
dedicated Technical Debt phase):** clean up the 20 pre-existing clippy
errors in cargo-pmcp so `cargo clippy -p cargo-pmcp --all-targets -- -D warnings`
is also green. Not in Wave 5 scope per `make quality-gate` being the authoritative gate.

## Threat flags

None added. Wave 5 adds no new network/auth/schema surface — only
observability (fuzz target), DX (example), and release metadata.

## Self-Check: PASSED

- `cargo-pmcp/fuzz/fuzz_targets/fuzz_iam_config.rs`: FOUND
- `cargo-pmcp/fuzz/corpus/fuzz_iam_config/seed_empty.toml`: FOUND
- `cargo-pmcp/fuzz/corpus/fuzz_iam_config/seed_cost_coach.toml`: FOUND
- `cargo-pmcp/fuzz/corpus/fuzz_iam_config/seed_wildcard.toml`: FOUND
- `cargo-pmcp/examples/deploy_with_iam.rs`: FOUND
- `cargo-pmcp/examples/fixtures/cost-coach.deploy.toml`: FOUND
- `cargo-pmcp/CHANGELOG.md`: FOUND
- Commit `b5e324d4`: FOUND (Task 1)
- Commit `98f835f1`: FOUND (Task 2)
- Commit `3923834a`: FOUND (Task 3)
- `cargo-pmcp/Cargo.toml` line 3 = `version = "0.10.0"`: FOUND
- `## IAM Declarations` in DEPLOYMENT.md: FOUND (line 1194)
- `[0.10.0]` in CHANGELOG.md: FOUND (line 8)
- `DEPLOYMENT.md` pointer in README.md: FOUND (line 22)
- `make quality-gate` exit code 0: PASSED
