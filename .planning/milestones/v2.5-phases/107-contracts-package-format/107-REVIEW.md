---
phase: 107-contracts-package-format
reviewed: 2026-07-18T03:21:54Z
depth: standard
files_reviewed: 10
files_reviewed_list:
  - .github/workflows/ci.yml
  - .github/workflows/release.yml
  - Makefile
  - Cargo.toml
  - crates/pmcp-package/Cargo.toml
  - contracts/team-servers-v1.yaml
  - crates/pmcp-package/tests/digest_stability.rs
  - crates/pmcp-package/tests/roundtrip.rs
  - crates/pmcp-package/tests/negative.rs
  - tests/team_contracts_conformance.rs
findings:
  critical: 0
  warning: 3
  info: 2
  total: 5
status: issues_found
---

# Phase 107: Code Review Report

**Reviewed:** 2026-07-18T03:21:54Z
**Depth:** standard
**Files Reviewed:** 10
**Status:** issues_found

## Summary

Reviewed the Phase 107 (Contracts & Package Format) authored surface: CI/release
wiring for the workspace-excluded `pmcp-package` leaf crate, the Makefile
`pmcp-package-gate` target, the root `Cargo.toml` publish `exclude` list, the
`contracts/team-servers-v1.yaml` provable-contract, the three `pmcp-package`
integration tests, and the root `team_contracts_conformance.rs` gate. The
adopted `crates/pmcp-package/src/**` tree was intentionally excluded from review.

Much of the wiring is correct and I verified it empirically rather than trusting
it:

- **Publish reachability is sound.** `pmcp-package` has its own empty
  `[workspace]` table and is absent from the root workspace `members`, so
  root `cargo fmt/clippy/test` genuinely ignore it; CI (ci.yml:95-102), the
  Makefile gate (Makefile:664-670), and release.yml:119-136 all correctly
  reach it via `--manifest-path` (a `-p pmcp-package` selector would not
  resolve). `pmcp-package-gate` is chained into `quality-gate`
  (Makefile:683).
- **`Cargo.toml` exclude is correct.** Both `contracts/` and the runtime-reading
  `tests/team_contracts_conformance.rs` are excluded (Cargo.toml:41-45), so the
  published `pmcp` crate does not ship the fixtures nor a test that would fail a
  downstream `cargo test`.
- **The wire-freeze is real, not merely deterministic.** I ran the suites: all
  four `EXPECTED_*_DIGEST` constants are genuine pinned `sha256:` hex asserted
  via `assert_eq!`, backed by a second `include_bytes!` canonical-bytes gate;
  `digest_stability`, `roundtrip`, and `negative` pass (8+4+... green), and the
  root conformance suite passes all 5 tests. A tampered deserialized field
  changes the digest → CI fails; a raw-byte reformat trips the `roundtrip`
  canonical-bytes assertion. The gates do what they claim for known fields.

The findings below are quality/robustness defects, not correctness breaks. No
blockers.

## Warnings

### WR-01: Release couples the core `pmcp` publish to an unconsumed leaf, and the ordering rationale cites a non-existent dependency

**File:** `.github/workflows/release.yml:83-136` (comments at 84-86, 122-125)
**Issue:** The publish step and its comments assert that `pmcp-package` "must
precede cargo-pmcp which pins it" / "cargo-pmcp (which pins `pmcp-package =
"0.1"`)". This dependency does not exist. A repo-wide search found **no** crate
depending on `pmcp-package` — `cargo-pmcp/Cargo.toml` contains zero references,
and nothing else in the workspace path-depends on it (that is precisely why it
needs the standalone `--manifest-path` gate). Two consequences:
1. The stated publish-order invariant is false, so a future maintainer reordering
   steps against that rationale has no real constraint to reason from.
2. Because the step is placed early (line 119, before `pmcp` core at line 213)
   and does `exit 1` on any non-"already exists" failure, a publish failure in
   this brand-new, **unconsumed** experimental leaf now aborts the job before the
   core SDK publishes. An unrelated leaf gates the core release for no dependency
   reason; since nothing consumes it, it could safely publish late (near
   `mcp-preview`/`cargo-pmcp`) to decouple it from core.
**Fix:** Either (a) move the `pmcp-package` publish step to the late tier
(after `pmcp` core, alongside the other consumer-less/CLI crates) and correct the
comment to state it is a standalone leaf with no in-repo consumers, or (b) if a
consumer is intended, add the actual `pmcp-package = "0.1"` dependency to the
consuming crate so the ordering claim becomes true and enforced by cargo.

### WR-02: Tool cache artifact committed under `contracts/`

**File:** `contracts/.pv/cache/lint/11385e98faa3b17d.json`
**Issue:** A `pv`/PMAT lint cache file is tracked in git (`git ls-files` lists
it; `git check-ignore` reports it is NOT ignored). This is machine-generated
tooling scratch, not contract content. It is excluded from the published crate
(the whole `contracts/` dir is excluded), so it is not a shipping bug, but it is
repo noise that will churn and produce spurious diffs. The root `Cargo.toml`
exclude comments (lines 36-39) already record that `.pmat`/`.pv` metadata must
be kept out of releases — the same rationale argues for keeping it out of git.
**Fix:** `git rm --cached contracts/.pv/cache/lint/11385e98faa3b17d.json` and add
`contracts/.pv/` (or a repo-wide `.pv/`) to `.gitignore`.

### WR-03: Conformance "cross-reference" is a raw substring grep, weaker than the suite's stated guarantee

**File:** `tests/team_contracts_conformance.rs:112-119, 121-149, 205-219`
**Issue:** `tool_is_captured` and `contract_declares_all_equations_and_tool_names`
match tool names with `contract.contains(name)` against the **entire YAML read as
a flat string** — the contract is never parsed into equations/invariants. The
module docstring claims it "cross-references every fixture's `request.name`
against the contract text" and asserts the advertised surface, but a tool name
appearing only in a prose comment or in the `metadata.description` blurb would
satisfy the check without being present in any equation's advertised
`static/dynamic tool` list. Conversely, substring containment can mask a genuine
enumeration gap when names overlap (the test happens to be safe for the current
19 names, but the guarantee is incidental, not designed). The gate is therefore
softer than advertised for the exact property Phase 107 exists to freeze — the
advertised tool surface.
**Fix:** Parse the YAML (a `serde_yaml`/`serde_json`-via-yaml step, or a minimal
line scan scoped to each `equations.<key>.formula` block) and assert each tool
name/prefix appears inside the relevant equation's advertised-tools section,
rather than anywhere in the file.

## Info

### IN-01: Fixture schema gate does not enforce outcome ↔ response shape

**File:** `tests/team_contracts_conformance.rs:151-202`
**Issue:** `fixtures_conform_to_versioned_schema` validates `schema_version`,
`case_id`, `server`, `request.name`, `expect.outcome` ∈ {success,error},
`expect.match` is a string, and `expect.response` is non-null — but never checks
that `outcome:"error"` fixtures carry `response.error` (with a numeric `code`)
and `outcome:"success"` fixtures carry `response.content`. All current fixtures
happen to follow that convention, so a malformed future fixture (e.g. an error
case with a `content` body) would pass the gate. This is partly a documented
Phase-109 deferral, but the outcome/response consistency is cheap to assert now.
**Fix:** In the loop, branch on `outcome`: for `"error"` assert
`v["expect"]["response"]["error"]["code"].is_number()`; for `"success"` assert
`v["expect"]["response"]["content"].is_array()`.

### IN-02: `pmcp-package` tests execute twice in CI

**File:** `.github/workflows/ci.yml:95-102` and `Makefile:672-689` (via
`quality-gate` → `pmcp-package-gate`)
**Issue:** The `test` job runs `cargo test --manifest-path
crates/pmcp-package/Cargo.toml` directly (ci.yml:102), and the `quality-gate` job
runs `make quality-gate`, which chains `pmcp-package-gate` running the same
`fmt`/`clippy`/`test` again. The two jobs run on separate runners so this is
duplicated compile+test work, not a correctness defect. Acceptable if
intentional (fail-fast in the `test` job); worth noting for CI-time budget.
**Fix (optional):** Drop the standalone step from the `test` job and rely on the
`quality-gate` job's `pmcp-package-gate`, or vice-versa.

---

_Reviewed: 2026-07-18T03:21:54Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
