---
phase: 107-contracts-package-format
plan: 02
subsystem: package-format
tags: [pmcp-package, wire-freeze, digest, golden-fixtures, quality-gate, ci, release, publish-order]

# Dependency graph
requires:
  - phase: 107-01
    provides: "crates/pmcp-package (publish-ready, workspace-excluded AI-Package format crate)"
  - phase: 107-03
    provides: "contracts/team-servers-v1.yaml + tests/team_contracts_conformance.rs"
provides:
  - "All-four-kind pinned canonical digests + canonical-JSON snapshots (real wire freeze, not just determinism)"
  - "agent + team golden fixtures (crates/pmcp-package/tests/golden_fixtures/)"
  - "Standalone pmcp-package quality gate (Makefile pmcp-package-gate + ci.yml step via --manifest-path)"
  - "release.yml early-leaf publish step for pmcp-package via --manifest-path with precise failure classification"
  - "Published-pmcp hygiene: tests/team_contracts_conformance.rs excluded from the pmcp crate"
  - "PKG-02 STATE-1 proof (dry-run publish + package-list) and recorded STATE-2 release follow-up"
affects:
  - "cargo-pmcp (Phase 110: pins pmcp-package = \"0.1\"; must publish after this leaf)"
  - "Release checkpoint (STATE-2 PKG-02: publish + cargo search + external consumer cargo check)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "pinned EXPECTED_<KIND>_DIGEST wire-freeze constant per serialized package kind"
    - "checked-in canonical-JSON snapshot (include_bytes! byte-equality) as second gate"
    - "workspace-excluded crate reached via --manifest-path in Makefile + ci.yml + release.yml"

key-files:
  created:
    - "crates/pmcp-package/tests/golden_fixtures/agent_pto_researcher_v1.json"
    - "crates/pmcp-package/tests/golden_fixtures/team_small_review_v1.json"
    - "crates/pmcp-package/tests/golden_fixtures/canonical/{server,workflow,agent,team}.canonical.json"
  modified:
    - "crates/pmcp-package/tests/digest_stability.rs"
    - "Makefile"
    - ".github/workflows/ci.yml"
    - ".github/workflows/release.yml"
    - "Cargo.toml"
    - "CLAUDE.md"
    - "crates/pmcp-package/src/** (rustfmt normalization on gate wiring)"

key-decisions:
  - "Input fixtures authored AS canonical bytes so the canonical snapshot for a kind equals its input fixture content (both gates share the same bytes, deserialize round-trips cleanly)"
  - "EXPECTED_<KIND>_DIGEST constants store the full sha256:<hex> form to match ManifestDigest::as_str()"
  - "Workspace [members]/[exclude] left UNCHANGED: cargo metadata --no-deps exits 0 and pmcp-package is already isolated by its own [workspace] table (A4 resolution)"

patterns-established:
  - "Pattern: every serialized package kind is guarded by BOTH a pinned digest and a canonical byte snapshot"
  - "Pattern: a workspace-excluded crate is gated locally and in CI only via --manifest-path, never -p"

requirements-completed: [PKG-02]

# Metrics
duration: ~35min
completed: 2026-07-18
---

# Phase 107 Plan 02: Wire-Freeze Teeth + pmcp-package Tooling Integration Summary

**All four serialized package kinds (server/workflow/agent/team) are now pinned by an `EXPECTED_<KIND>_DIGEST` constant AND a checked-in canonical-JSON snapshot, and the workspace-excluded `pmcp-package` crate is fmt/clippy/test-gated locally + in CI and wired into release.yml as an early leaf via `--manifest-path`.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-07-18T02:38Z (approx)
- **Completed:** 2026-07-18T03:13Z
- **Tasks:** 3
- **Files modified:** 30 (7 Task 1 + 24 Task 2, incl. rustfmt normalization; SUMMARY in Task 3)

## Accomplishments

- **Real wire freeze, not just determinism (Codex HIGH closed):** authored `agent_pto_researcher_v1.json` + `team_small_review_v1.json` golden fixtures and, for ALL FOUR kinds, added a pinned `EXPECTED_<KIND>_DIGEST` assertion (`manifest_digest(value).as_str()`) plus a byte-equal `canonicalize()` snapshot assertion against `tests/golden_fixtures/canonical/<kind>.canonical.json`. The ≥100-recomputation determinism assertions are retained per kind. Spot-checked: mutating `max_tokens` in the agent fixture flips the digest and FAILS the pinned test.
- **Closed the workspace-exclusion quality blind spot (both reviewers HIGH):** new `pmcp-package-gate` Makefile target (fmt --check + clippy --all-targets -D warnings + test via `--manifest-path`) chained into `quality-gate`, and a matching "pmcp-package standalone checks" step in `ci.yml`'s `test` job.
- **Published-`pmcp` hygiene (Gemini MEDIUM):** added `tests/team_contracts_conformance.rs` to the root `Cargo.toml` `[package] exclude` list so the conformance test (which reads the already-excluded `contracts/`) never ships and can't break a downstream `cargo test`.
- **Release wiring:** `release.yml` gains an early-leaf `Publish pmcp-package` step using `cargo publish --manifest-path crates/pmcp-package/Cargo.toml` (NOT `-p`), continuing only on an "already exists" match and `exit 1`ing on all other failures, with a `sleep 30` index wait. CLAUDE.md publish order documents it as leaf `1b`.
- **PKG-02 STATE-1 proven:** `cargo publish --dry-run --allow-dirty` exits 0 and `cargo package --list` includes README.md, CHANGELOG.md, LICENSE-MIT, LICENSE-APACHE.

## Task Commits

1. **Task 1: All-four-kind golden fixtures with pinned digests + snapshots** — `3aa09737` (test)
2. **Task 2: Wire pmcp-package into repo tooling (gates, publish, hygiene)** — `ee217347` (chore)
3. **Task 3: PKG-02 STATE-1 acceptance + STATE-2 record** — this SUMMARY (committed in the metadata commit)

_Task 1 is the TDD RED+GREEN authoring step; the fixtures and pinned constants land together because the pinned value can only exist once the canonical bytes are authored._

## Files Created/Modified

- `crates/pmcp-package/tests/golden_fixtures/agent_pto_researcher_v1.json` — AgentPackage golden fixture (canonical-JSON input)
- `crates/pmcp-package/tests/golden_fixtures/team_small_review_v1.json` — TeamPackage golden fixture (canonical-JSON input)
- `crates/pmcp-package/tests/golden_fixtures/canonical/{server,workflow,agent,team}.canonical.json` — checked-in canonical byte snapshots (secondary gate)
- `crates/pmcp-package/tests/digest_stability.rs` — pinned digest + snapshot + round-trip + determinism tests for all four kinds (17 tests)
- `Makefile` — `pmcp-package-gate` target + chain into `quality-gate`
- `.github/workflows/ci.yml` — standalone pmcp-package fmt/clippy/test step
- `.github/workflows/release.yml` — early-leaf pmcp-package publish step via `--manifest-path`
- `Cargo.toml` — exclude `tests/team_contracts_conformance.rs` from the published pmcp crate
- `CLAUDE.md` — pmcp-package leaf entry (`1b`) in publish order
- `crates/pmcp-package/src/**` — rustfmt normalization applied now that standalone fmt is gated (incl. the pre-existing `canonical.rs` `strip_prefix` diff flagged in 107-01)

## PKG-02 Two-State Acceptance (explicit, per Codex HIGH — NOT silently deferred)

PKG-02 completion is defined as TWO states:

- **STATE 1 — proven here, in-phase (publish-ready + release-wired + quality-gated + wire-frozen):**
  - `cargo publish --dry-run --allow-dirty` from `crates/pmcp-package` exits 0.
  - `cargo package --list` contains README.md, CHANGELOG.md, LICENSE-MIT, LICENSE-APACHE.
  - All four kinds are pinned-digest + canonical-snapshot guarded (`cargo test --test digest_stability` = 17 passed).
  - `pmcp-package-gate` runs fmt/clippy/test via `--manifest-path` and is chained into `quality-gate`; ci.yml mirrors it.
  - release.yml reaches the crate via `--manifest-path` as an early leaf.

- **STATE 2 — REQUIRED release-checkpoint follow-up, OUT of this phase (published + externally resolvable):**
  1. The tag-triggered `release.yml` runs `cargo publish --manifest-path crates/pmcp-package/Cargo.toml`.
  2. `cargo search pmcp-package --limit 1` reports `0.1.0` on crates.io.
  3. A throwaway consumer crate with `pmcp-package = "0.1"` (NO path override) runs `cargo check` successfully.

  **PKG-02 MUST NOT be marked Shipped until STATE 2 passes.** This is a real published-artifact gate, not a dry-run; it can only be exercised after a `v*` tag publish, so it is recorded here as a mandatory release-checkpoint item rather than executed from a plan (publishes never run from a plan — repo precedent).

## Decisions Made

- **Input fixtures authored as canonical bytes.** Producing each new fixture via `canonicalize(&value)` means the input fixture and its `canonical/<kind>.canonical.json` snapshot share identical bytes; the existing server/workflow input fixtures were already canonical (verified byte-equal), so all four kinds are internally consistent and deserialize round-trips are exact.
- **Digest constants use the full `sha256:<hex>` form** to match `ManifestDigest::as_str()` (the plan's "<64-hex>" was loose wording; the assertion is against `as_str()`).
- **Workspace `[members]`/`[exclude]` unchanged (A4).** `cargo metadata --format-version 1 --no-deps` exits 0 and `pmcp-package` is not in the workspace package set — it is already isolated by its own `[workspace]` table, so no `exclude` entry was required.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Reworded a release.yml comment that tripped the verify guard**
- **Found during:** Task 2 (running the plan's verify command).
- **Issue:** An explanatory comment in the new publish step contained the literal phrase `cargo publish -p pmcp-package`, which the verify assertion `! grep -q "cargo publish -p pmcp-package"` treats as a forbidden `-p` invocation — the guard cannot distinguish a comment from a command.
- **Fix:** Reworded the comment to "the `-p <name>` package selector does NOT resolve it" so no line contains the forbidden literal while preserving the explanation.
- **Files modified:** `.github/workflows/release.yml`
- **Verification:** Full Task 2 verify command passes ("ALL TASK2 VERIFY CHECKS PASS").
- **Committed in:** `ee217347` (Task 2 commit)

**2. [Rule 1 - Convention] rustfmt-normalized the whole pmcp-package crate on gate wiring**
- **Found during:** Task 2 (running the new `pmcp-package-gate` fmt --check).
- **Issue:** The crate was never fmt-gated before (workspace-excluded), so `cargo fmt --check` reported diffs beyond the pre-flagged `canonical.rs` one (16 src/test files). Leaving them would make the new gate fail on its first CI run.
- **Fix:** Ran `cargo fmt --manifest-path crates/pmcp-package/Cargo.toml --all` to normalize. This is the expected consequence of wiring the standalone fmt gate (107-01 explicitly deferred this normalization to this plan). No behavioral change; clippy clean; 130 crate tests pass.
- **Files modified:** `crates/pmcp-package/src/**` + `tests/{digest_stability,negative,roundtrip}.rs`
- **Verification:** `pmcp-package-gate` fmt/clippy/test all pass.
- **Committed in:** `ee217347` (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 convention).
**Impact on plan:** Both are mechanical consequences of the plan's own instructions (verify-guard literal; gate wiring normalizing an ungated crate). No scope creep.

## Issues Encountered

- The `rtk` cargo proxy swallowed `--nocapture` stdout during fixture generation, so the pinned digests were computed directly as `shasum -a 256` of the checked-in canonical snapshots (identical to `manifest_digest`, which is SHA-256 of the canonical bytes). Verified consistent by the passing `*_digest_matches_pinned_wire_freeze_constant` tests.

## Next Phase Readiness

- Wire freeze has real teeth across all four kinds; any serialized-shape change now fails CI (the intentional 0.2.0 trigger).
- `pmcp-package` is gated and release-wired; Phase 110's `cargo-pmcp` can pin `pmcp-package = "0.1"` once STATE-2 publish lands.
- **Release checkpoint owner must execute PKG-02 STATE-2 before marking PKG-02 Shipped.**

## Threat Flags

None. All work maps to the plan's existing threat register (T-107-04/05/10/11 mitigated; T-107-SC accepted — no new packages introduced).

## Self-Check: PASSED

Files verified present: agent/team fixtures, four canonical snapshots, digest_stability.rs, Makefile/ci.yml/release.yml/Cargo.toml/CLAUDE.md edits, 107-02-SUMMARY.md.
Commits verified in git log: `3aa09737` (Task 1), `ee217347` (Task 2).

---
*Phase: 107-contracts-package-format*
*Completed: 2026-07-18*
