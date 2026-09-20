---
phase: 107-contracts-package-format
verified: 2026-07-18T03:23:53Z
status: human_needed
score: 3/3 must-haves verified
overrides_applied: 0
human_verification:
  - test: "PKG-02 STATE-2: publish pmcp-package to crates.io via a v* tag push, then confirm `cargo search pmcp-package --limit 1` reports 0.1.0, then create a throwaway consumer crate depending on `pmcp-package = \"0.1\"` (no path override) and run `cargo check` successfully"
    expected: "pmcp-package 0.1.0 is live on crates.io and externally resolvable by a downstream Cargo.toml dependency (not just a local path)"
    why_human: "Publishing to crates.io only happens from the tag-triggered release.yml workflow, never from a plan or verification run (repo release precedent recorded in CLAUDE.md and 107-02-SUMMARY.md). This is a release-checkpoint action gated on a maintainer pushing a v* tag, not something the verifier can or should execute. 107-02-SUMMARY.md explicitly states 'PKG-02 MUST NOT be marked Shipped until STATE 2 passes' — this item tracks that required follow-up so it isn't silently dropped."
---

# Phase 107: Contracts & Package Format Verification Report

**Phase Goal:** The portability contracts exist, versioned and wire-frozen, with this repo as the canonical home — `pmcp-package` adopted (from `~/Development/mcp/sdk/pmcp-run/crates/pmcp-package`) and published 0.1.0, plus the four team servers' tool surfaces captured as provable-contracts YAML with shared conformance fixtures.
**Verified:** 2026-07-18T03:23:53Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | PKG-01: `pmcp-package` builds in this repo as a standalone workspace-excluded crate with publish-ready metadata (public description, README, license files, docs.rs-verified rustdoc) | ✓ VERIFIED | `cd crates/pmcp-package && cargo test` → 130 passed (0 failed). `Cargo.toml` has `[workspace]` first, `repository = "https://github.com/paiml/rust-mcp-sdk"`, `[package.metadata.docs.rs]` table. `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps` exits 0. Root workspace `members` list (Cargo.toml:582) does NOT contain `crates/pmcp-package`; `cargo metadata --no-deps` at repo root exits 0. Scrub-regex loop over full `cargo package --list` file set returns zero matches for internal planning refs. |
| 2 | PKG-02: a developer can depend on `pmcp-package = "0.1"` from crates.io; wire-freeze policy documented and enforced by passing golden fixtures | ✓ VERIFIED (in-phase STATE-1 scope; STATE-2 publish is a release-checkpoint follow-up — see Human Verification) | `cargo publish --dry-run --allow-dirty` from `crates/pmcp-package` exits 0; `cargo package --list` contains README.md/CHANGELOG.md/LICENSE-MIT/LICENSE-APACHE. `cargo test --test digest_stability` → 17 passed, covering all 4 kinds (server/workflow/agent/team) with pinned `EXPECTED_<KIND>_DIGEST` + canonical-snapshot + ≥100-recomputation assertions. **Live tamper spot-check performed by this verifier**: mutated `max_tokens` in `agent_pto_researcher_v1.json` → both `agent_canonical_bytes_match_checked_in_snapshot` and `agent_fixture_digest_matches_pinned_wire_freeze_constant` FAILED as expected (real wire-freeze, not just determinism); fixture restored and re-verified clean (17/17 pass again). `make pmcp-package-gate` exits 0 (fmt/clippy/test via `--manifest-path`). README documents the 0.1.x wire-freeze policy explicitly. `.github/workflows/release.yml` has an early-leaf `Publish pmcp-package` step using `--manifest-path` (not `-p`) with precise "already exists" vs `exit 1` failure classification. `crates.io` does not yet list `pmcp-package` (checked live) — expected, per scope context this is a release-checkpoint (STATE-2) action, not an in-phase deliverable. |
| 3 | PKG-03: team-server tool contracts (`fs__*`, `mem__*`, `team_mcp__<member>` dispatch, `resolve_approval`/`get_approval` + `team_approval__ask_*`) captured as versioned provable-contracts YAML with shared conformance fixtures, marked as namespaced provisional PMCP extensions | ✓ VERIFIED | `contracts/team-servers-v1.yaml` parses as valid YAML with `equations: {fs_tool_surface, mem_tool_surface, approval_tool_surface, team_dispatch_surface}`; contains all 19 static tool names + both dynamic-family prefixes; `metadata.description` contains "provisional"/"extension" and calls out `resolve_approval`/`get_approval` as unnamespaced legacy names; zero `lean_theorem`/`DynamoDB`/`DDB` occurrences (all confirmed by a live re-run of the plan's own python assertion script). `cargo test --test team_contracts_conformance` (also re-run with `--all-features`) → 5 passed: schema validation, tool cross-reference, per-server + negative coverage (5 negatives ≥ required 4), and the `related_task`-under-top-level-`_meta` assertion. 13 fixtures across 4 server directories inspected directly — all conform to the versioned schema and match claimed content (positive per family, both dynamic families, 5 negative/security cases). No `contracts/team-servers-binding.yaml` exists; the pre-existing unrelated `contracts/binding.yaml` (predates this phase, binds `mcp-protocol-sdk-v1.yaml`) contains zero references to the team-servers equations. |

**Score:** 3/3 truths verified (all ROADMAP Success Criteria for Phase 107 hold in the codebase). Status is `human_needed`, not `passed`, solely because PKG-02's own two-state definition (recorded explicitly in 107-02-SUMMARY.md) requires a release-checkpoint action (actual crates.io publish + external-consumer resolution) that cannot and should not be executed from this verification pass — see Human Verification below.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-package/Cargo.toml` | Publish-ready package metadata incl. docs.rs table | ✓ VERIFIED | `repository = "https://github.com/paiml/rust-mcp-sdk"`, `readme`, `keywords` (5), `categories`, `[package.metadata.docs.rs]` all present; pinned deps unchanged |
| `crates/pmcp-package/src/lib.rs` | Ported crate root (scrubbed rustdoc) | ✓ VERIFIED | No `Phase N`/`Wave N`/ticket-ID refs; rustdoc builds clean under `-D rustdoc::broken_intra_doc_links` |
| `crates/pmcp-package/README.md` | Public overview + wire-freeze policy | ✓ VERIFIED | Explicit "Wire-Freeze Policy" section: 0.1.x stable, pinned-digest enforced, shape change → 0.2.0 |
| `crates/pmcp-package/LICENSE-MIT` / `LICENSE-APACHE` | Dual-license resolution | ✓ VERIFIED | Both present in `cargo package --list`; LICENSE-APACHE byte-identical canonical text (0 occurrences of "Pragmatic AI Labs") |
| `crates/pmcp-package/NOTICE` | Ownership/attribution notice | ✓ VERIFIED | Contains "Copyright 2025 Pragmatic AI Labs", SPDX note, kept out of license bodies |
| `crates/pmcp-package/tests/golden_fixtures/agent_pto_researcher_v1.json`, `team_small_review_v1.json` | AgentPackage/TeamPackage golden fixtures | ✓ VERIFIED | Both deserialize; both used in pinned-digest + snapshot + 100x-determinism tests |
| `crates/pmcp-package/tests/golden_fixtures/canonical/*.canonical.json` | Checked-in canonical-JSON snapshots (one per kind) | ✓ VERIFIED | `server.canonical.json`, `workflow.canonical.json`, `agent.canonical.json`, `team.canonical.json` all present; byte-equality assertions pass and correctly fail on tamper (live-tested) |
| `Makefile` (`pmcp-package-gate`) | Standalone pmcp-package quality gate wired into `quality-gate` | ✓ VERIFIED | `pmcp-package-gate` target runs fmt/clippy/test via `--manifest-path`; chained into `quality-gate` (line 683); `make pmcp-package-gate` exits 0 |
| `.github/workflows/ci.yml` | CI standalone pmcp-package fmt/clippy/test step | ✓ VERIFIED | "pmcp-package standalone checks" step present after "Run tests", using `--manifest-path` |
| `.github/workflows/release.yml` | crates.io publish step for pmcp-package | ✓ VERIFIED | Early-leaf "Publish pmcp-package" step right after widget-utils, `--manifest-path` (no `-p`), precise "already exists" vs `exit 1` classification, `sleep 30` index wait |
| `contracts/team-servers-v1.yaml` | Provable-contracts YAML for the four team-server tool surfaces | ✓ VERIFIED | 4 equations, all 19 static names + 2 dynamic prefixes, storage-agnostic, no `lean_theorem` |
| `tests/team_contracts_conformance.rs` | Schema-aware conformance gate over contract + versioned fixtures | ✓ VERIFIED | 5 substantive tests (not stubs) — schema validation, tool cross-ref, per-server/negative coverage counts, `_meta.related_task` presence; passes with `--all-features` |
| `contracts/team-servers/fixtures/{team-fs,mem-mcp,approval-mcp,team-mcp}` | Versioned request/expect conformance fixtures | ✓ VERIFIED | 13 fixtures total across 4 directories; every fixture inspected directly, schema-conformant, tool names present in contract |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `crates/pmcp-package/README.md` | `crates/pmcp-package/tests/digest_stability.rs` | Wire-freeze policy documents what the golden-fixture pinned-digest test mechanically enforces | ✓ WIRED | README text matches the actual test mechanics (pinned `EXPECTED_<KIND>_DIGEST` + canonical snapshot); confirmed by tamper spot-check |
| `crates/pmcp-package/tests/digest_stability.rs` | `crates/pmcp-package/tests/golden_fixtures/agent_pto_researcher_v1.json` (+ team/server/workflow) | Pinned-digest test deserializes the fixture and asserts its canonical digest equals a checked-in constant | ✓ WIRED | Live tamper test: mutating the fixture flips the digest and fails the pinned assertion for all 4 kinds' pattern (spot-checked on `agent`) |
| `.github/workflows/release.yml` | `crates/pmcp-package/Cargo.toml` | `cargo publish --manifest-path crates/pmcp-package/Cargo.toml` | ✓ WIRED | Confirmed present; `-p pmcp-package` confirmed absent (only appears inside an explanatory comment, correctly reworded per 107-02-SUMMARY deviation note) |
| `Makefile` | `crates/pmcp-package/Cargo.toml` | `quality-gate` target runs fmt/clippy/test via `--manifest-path` | ✓ WIRED | `pmcp-package-gate` chained at line 683; ran it directly, exits 0 |
| `tests/team_contracts_conformance.rs` | `contracts/team-servers-v1.yaml` | Test reads the YAML text via `CARGO_MANIFEST_DIR` and asserts equation keys + tool-name literals | ✓ WIRED | `contract_declares_all_equations_and_tool_names` test passes; verified by direct file read |
| `tests/team_contracts_conformance.rs` | `contracts/team-servers/fixtures` | Test parses every versioned fixture and cross-references `request.name` against the contract | ✓ WIRED | `every_fixture_tool_is_captured_in_contract` + `fixtures_conform_to_versioned_schema` tests pass; fixtures manually inspected and match |

### Data-Flow Trace (Level 4)

Not applicable in the conventional sense (no UI/API rendering pipeline in this phase — the phase produces a portable-format crate and static contract/fixture data). The equivalent "does the pinned data actually flow through the enforcement mechanism" check was performed via the live tamper spot-check on `agent_pto_researcher_v1.json`: mutating a real field flowed through `manifest_digest()`/`canonicalize()` and correctly failed both the pinned-digest and canonical-snapshot tests, then the fixture was restored and 17/17 passed again cleanly (`git status --porcelain` on the fixture confirms no residual diff).

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| pmcp-package crate builds and tests standalone | `cd crates/pmcp-package && cargo test` | 130 passed, 0 failed | ✓ PASS |
| Wire-freeze digest tests hold for all 4 kinds | `cargo test --test digest_stability` | 17 passed | ✓ PASS |
| Wire-freeze actually catches drift (not just determinism) | mutate `agent_pto_researcher_v1.json` field, rerun `cargo test --test digest_stability`, restore | 2 tests failed as expected on mutation; 17/17 pass after restore | ✓ PASS |
| `cargo publish --dry-run` packaging is valid | `cargo publish --dry-run --allow-dirty` (crates/pmcp-package) | Exit 0, 43 files packaged incl. README/CHANGELOG/both licenses | ✓ PASS |
| docs.rs-clean rustdoc | `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps` | Exit 0 | ✓ PASS |
| Standalone quality gate wired and passing | `make pmcp-package-gate` | Exit 0 (fmt/clippy/test all pass) | ✓ PASS |
| Root workspace still resolves with pmcp-package excluded | `cargo metadata --format-version 1 --no-deps` | Exit 0 | ✓ PASS |
| PKG-03 conformance test | `cargo test --test team_contracts_conformance` and `--all-features` variant | 5 passed both times | ✓ PASS |
| Repo-wide clippy on the conformance test file under the CI allow-list | `cargo clippy --test team_contracts_conformance --all-targets --all-features -- -D warnings <CI allow-list>` | Clean, 0 warnings | ✓ PASS |
| Repo-wide `cargo fmt --all -- --check` | `cargo fmt --all -- --check` | Exit 0 | ✓ PASS |
| Internal planning ref scrub over the full packaged file set | Generic-regex loop over `cargo package --list --allow-dirty` | 0 matching files | ✓ PASS |
| Anti-pattern scan (TBD/FIXME/XXX/TODO/HACK/PLACEHOLDER) | `grep -rn` over all phase-touched files | 0 hits (Makefile hits are its own pre-existing `check-todos` target description, unrelated) | ✓ PASS |

### Probe Execution

Step 7c SKIPPED — no `scripts/*/tests/probe-*.sh` convention applies to this phase; the PLAN/SUMMARY files do not declare any probe scripts. Verification instead used direct `cargo test`/`cargo publish --dry-run`/`make pmcp-package-gate` invocations as documented above.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| PKG-01 | 107-01-PLAN.md | `pmcp-package` lives in this repo as its canonical home (standalone workspace-excluded crate) with publish-ready metadata | ✓ SATISFIED | See Truth #1 / Artifacts table |
| PKG-02 | 107-02-PLAN.md | `pmcp-package` 0.1.0 is published to crates.io with the wire-freeze policy documented | ✓ SATISFIED (in-phase STATE-1) — STATE-2 (actual crates.io publish) is a required, explicitly-recorded release-checkpoint follow-up, not yet executed | See Truth #2; Human Verification section |
| PKG-03 | 107-03-PLAN.md | Team-server tool contracts captured as versioned provable-contracts YAML with shared conformance fixtures | ✓ SATISFIED | See Truth #3 / Artifacts table |

No orphaned requirements: all three PKG-* requirements assigned to Phase 107 in REQUIREMENTS.md are claimed by exactly one plan each (107-01/PKG-01, 107-02/PKG-02, 107-03/PKG-03). TEAM-06 (REQUIREMENTS.md line 43) is assigned to Phase 109, not Phase 107, and is correctly out of scope here — Phase 107 only ships the shared fixtures TEAM-06 will later execute against.

### Anti-Patterns Found

None. Scanned all phase-touched files (`crates/pmcp-package/**`, `contracts/team-servers-v1.yaml`, `contracts/team-servers/fixtures/**`, `tests/team_contracts_conformance.rs`, `Makefile`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `CLAUDE.md`, root `Cargo.toml`) for debt markers (`TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER`), empty-implementation patterns, and hardcoded-empty-return patterns. Zero hits beyond the Makefile's own pre-existing `check-todos` target description text (not a marker itself).

The one documented deviation (107-01-SUMMARY.md: relaxing the repo-coupled `deploy.toml` fixture-coverage floor from a hardcoded `>= 15` to "every discovered file parses") was inspected directly in `src/package/server.rs` — it is a legitimate environment-agnostic invariant rewrite (not a weakened/stubbed test), documented with rationale in the source comments.

### Human Verification Required

### 1. PKG-02 STATE-2: crates.io publish + external resolvability

**Test:** Push a `v*` release tag so `.github/workflows/release.yml` runs; confirm the "Publish pmcp-package" step succeeds (not skipped by an "already exists" false match). Then run `cargo search pmcp-package --limit 1` and confirm it reports `0.1.0`. Then create a throwaway crate with `pmcp-package = "0.1"` in its `[dependencies]` (no `path =` override) and run `cargo check` against it.
**Expected:** `pmcp-package` 0.1.0 is live on crates.io, indexed, and resolvable as a normal registry dependency by a project with no knowledge of this repo's local path layout.
**Why human:** This is a release-checkpoint action, not an in-phase deliverable — publishing to crates.io only happens from the tag-triggered `release.yml` workflow per repo precedent (never from a plan or a verification pass; live check during this verification confirmed `pmcp-package` is not yet on crates.io, as expected). `107-02-SUMMARY.md` explicitly names this "STATE 2" and states "PKG-02 MUST NOT be marked Shipped until STATE 2 passes" — this human-verification item exists so that required follow-up is tracked rather than silently dropped when this phase is marked complete.

### Gaps Summary

No gaps. All three ROADMAP Success Criteria for Phase 107 (PKG-01, PKG-02, PKG-03) are verified as observably true in the codebase, backed by live command execution (not SUMMARY-claim trust) including a destructive tamper-and-restore spot-check that proves the wire-freeze mechanism has real teeth. The only reason this report is not `status: passed` is the explicit, self-documented PKG-02 two-state acceptance: this phase proves publish-readiness (STATE 1) but the actual crates.io publish (STATE 2) is a release-checkpoint action correctly deferred outside this phase's plans and surfaced above as a human-verification item so it is not lost.

---

*Verified: 2026-07-18T03:23:53Z*
*Verifier: Claude (gsd-verifier)*
