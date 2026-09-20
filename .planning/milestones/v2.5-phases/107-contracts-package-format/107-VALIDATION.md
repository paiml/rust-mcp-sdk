---
phase: 107
slug: contracts-package-format
status: planned
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-17
updated: 2026-07-18
revision: reviews-incorporated
---

# Phase 107 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Revised after cross-AI review (107-REVIEWS.md) — task set expanded per the 6 consensus items.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | crates/pmcp-package/Cargo.toml (after adoption) |
| **Quick run command** | `cargo test --manifest-path crates/pmcp-package/Cargo.toml` |
| **Full suite command** | `make quality-gate` (now includes the `pmcp-package-gate` standalone target) |
| **Estimated runtime** | ~60 seconds (crate-local); quality-gate several minutes |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --manifest-path crates/pmcp-package/Cargo.toml` (pmcp-package tasks) or `cargo test --test team_contracts_conformance` (Plan 03)
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-T1 | 107-01 | 1 | PKG-01 | T-107-01 | crate builds standalone, unedited | build/unit | `cd crates/pmcp-package && cargo test` | ✅ (ported tests) | ⬜ pending |
| 01-T2 | 107-01 | 1 | PKG-01 | T-107-01/09 | publish metadata + docs.rs table + canonical license/NOTICE; dry-run (failure-preserving) + package-list shows README/CHANGELOG/both licenses | publish dry-run | `cd crates/pmcp-package && cargo publish --dry-run --allow-dirty && cargo package --list --allow-dirty \| grep -q '^LICENSE-APACHE$'` | ✅ (cargo built-in) | ⬜ pending |
| 01-T3 | 107-01 | 1 | PKG-01 | T-107-01 | no internal ticket ID leaks to ANY packaged file (generic regex) | doc-lint | `cd crates/pmcp-package && test $(cargo package --list --allow-dirty \| while read f; do [ -f "$f" ] && grep -lE "(Phase [0-9]+\|Wave [0-9]+\|I-[0-9]+\|D-[0-9]+\|T-[0-9]+\|guyernest/pmcp-run)" "$f"; done \| wc -l) -eq 0` + `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps` | ❌ W0 (doc-lint gate) | ⬜ pending |
| 02-T1 | 107-02 | 2 | PKG-02 | T-107-04 | all 4 kinds pinned-digest + canonical-snapshot guarded (wire freeze, not just determinism) | golden-fixture | `cd crates/pmcp-package && cargo test --test digest_stability` | ❌ W0 (add agent_/team_ fixtures + EXPECTED_*_DIGEST + canonical snapshots) | ⬜ pending |
| 02-T2 | 107-02 | 2 | PKG-02 | T-107-05/10/11 | excluded crate fmt/clippy/test-gated (Makefile+CI); conformance test excluded from published pmcp; release publishes via manifest-path | integration | `cargo metadata --format-version 1 --no-deps` + `grep -q "manifest-path crates/pmcp-package/Cargo.toml" Makefile .github/workflows/ci.yml .github/workflows/release.yml` + `grep -q "tests/team_contracts_conformance.rs" Cargo.toml` | ❌ W0 (wire Makefile/ci/release + exclude) | ⬜ pending |
| 02-T3 | 107-02 | 2 | PKG-02 | T-107-05 | PKG-02 STATE 1 proven; STATE 2 (publish+resolve) recorded as required follow-up | publish-readiness | `cd crates/pmcp-package && cargo publish --dry-run --allow-dirty && cargo package --list --allow-dirty \| grep -q '^README.md$'` | ✅ (cargo built-in) | ⬜ pending |
| 03-T1 | 107-03 | 1 | PKG-03 | T-107-06/07/12 | contract captures 4 surfaces + correct dispatch invariants; storage-agnostic; no lean_theorem | yaml-parse | `python3 -c "import yaml; yaml.safe_load(open('contracts/team-servers-v1.yaml'))"` + key/tool-name/no-DDB/no-lean_theorem asserts | ❌ W0 (author contract) | ⬜ pending |
| 03-T2 | 107-03 | 1 | PKG-03 | T-107-06/07 | versioned fixtures (positive per family + both dynamic + high-value negatives); schema + cross-ref enforced; CARGO_MANIFEST_DIR paths | conformance | `cargo test --test team_contracts_conformance` | ❌ W0 (author versioned fixtures + schema-aware test) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-package/tests/digest_stability.rs` — extend golden fixtures to agent + team kinds AND pin `EXPECTED_<KIND>_DIGEST` + canonical snapshots for all four kinds
- [ ] `crates/pmcp-package/tests/golden_fixtures/canonical/*.canonical.json` — checked-in canonical snapshots (one per kind)
- [ ] `Makefile` + `.github/workflows/ci.yml` — standalone `pmcp-package` fmt/clippy/test via `--manifest-path`
- [ ] Root `Cargo.toml` exclude — add `tests/team_contracts_conformance.rs`
- [ ] `contracts/team-servers-v1.yaml` + versioned conformance fixtures (positive + negative)
- [ ] `tests/team_contracts_conformance.rs` — schema-aware, CARGO_MANIFEST_DIR-resolved
- [ ] No framework install needed — cargo test is built-in

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| docs.rs rendering of published crate | PKG-01 | docs.rs builds post-publish | `cargo doc --no-deps` locally as proxy; verify docs.rs after publish |
| crates.io publish + external resolution (PKG-02 STATE 2) | PKG-02 | requires actual publish | Release checkpoint: after tag-triggered `release.yml` publish, `cargo search pmcp-package` shows 0.1.0, then a throwaway crate with `pmcp-package = "0.1"` (no path) runs `cargo check`. PKG-02 not Shipped until this passes. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
