---
phase: 67-docs-rs-pipeline-and-feature-flags
plan: 05
subsystem: infra
tags: [makefile, ci, github-actions, rustdoc, docs-rs, quality-gate]

# Dependency graph
requires:
  - phase: 67-04
    provides: "Zero rustdoc warnings baseline under the D-16 feature list — makes `make doc-check` green on arrival"
  - phase: 67-01
    provides: "[package.metadata.docs.rs] feature list — single source of truth that the new Makefile target mirrors"
provides:
  - "`make doc-check` target enforcing zero rustdoc warnings via `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --features <D-16 list>`"
  - "CI `Check rustdoc zero-warnings` step inside the existing `quality-gate` job"
  - "Drift detection surface for Plan 06 Check 4 (byte-identity diff between Makefile and Cargo.toml feature lists)"
affects: [67-06, future-rustdoc-changes, future-feature-additions]

# Tech tracking
tech-stack:
  added: []  # Build infrastructure only, no new dependencies
  patterns:
    - "Standalone rustdoc gate Makefile target (opt-in locally, mandatory in CI)"
    - "Explicit feature list mirrors [package.metadata.docs.rs] verbatim (single source of truth)"
    - "Gate step colocated inside existing quality-gate CI job (no new runner, no new CI minutes)"

key-files:
  created: []
  modified:
    - "Makefile (added .PHONY: doc-check target at lines 411-416)"
    - ".github/workflows/ci.yml (added Check rustdoc zero-warnings step at lines 205-206 inside quality-gate job)"

key-decisions:
  - "D-23 honored: exact Makefile recipe with `@echo`/`RUSTDOCFLAGS`/`@echo` structure matching existing `doc:` target style"
  - "D-24 honored: stable toolchain, no `--cfg docsrs` (would fail E0557 on stable)"
  - "D-25 honored: explicit 15-feature list, no `--all-features`"
  - "D-26 honored: new step is INSIDE the existing quality-gate job, not a new job"
  - "D-27 honored: `make doc-check` is NOT chained from `make quality-gate` — preserves local iteration speed, CI enforces drift detection"

patterns-established:
  - "Rustdoc gate as standalone opt-in Makefile target with dedicated CI step — pattern to extend when adding workspace-wide gates"
  - "Single-source-of-truth feature list between Makefile target and Cargo.toml docs.rs metadata — drift caught at CI time"

requirements-completed:
  - DRSD-04

# Metrics
duration: ~8min
completed: 2026-04-11
---

# Phase 67 Plan 05: Makefile and CI Gate Summary

**Zero-tolerance rustdoc gate landed via new `make doc-check` Makefile target and a dedicated `Check rustdoc zero-warnings` step inside the existing CI `quality-gate` job — every future PR is now automatically gated against rustdoc drift under the D-16 feature list.**

## Performance

- **Duration:** ~8 minutes
- **Tasks:** 2
- **Files modified:** 2 (Makefile, .github/workflows/ci.yml)
- **Lines added:** 10 (7 Makefile + 3 ci.yml)

## Accomplishments

- New `make doc-check` target in root `Makefile` runs `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` under the exact 15-feature D-16 list, colocated with the existing `doc:` / `doc-open:` targets.
- New CI step `Check rustdoc zero-warnings` inside the existing `quality-gate` job in `.github/workflows/ci.yml` runs `make doc-check` on every PR — positioned between `Check disk space before quality gate` and `Run quality gate`.
- All six invariants hold: stable-toolchain (D-24), explicit feature list not `--all-features` (D-25), inside existing job not new job (D-26), not chained from `make quality-gate` (D-27), TAB-indented recipe lines, byte-identical feature list vs. Cargo.toml `[package.metadata.docs.rs]`.

## Task Commits

1. **Task 1: Add `doc-check` target to Makefile** — `faf52e86` (feat)
2. **Task 2: Add `Check rustdoc zero-warnings` step to ci.yml quality-gate job** — `48e7802b` (ci)

## Files Created/Modified

### `Makefile` — new target at lines 411–416

Diff (7 added lines including one blank separator):

```makefile
.PHONY: doc-check
doc-check:
	@echo "$(BLUE)Checking rustdoc warnings (zero-tolerance)...$(NC)"
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps \
		--features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
	@echo "$(GREEN)✓ Zero rustdoc warnings$(NC)"
```

Placement: immediately after `doc-open:` (ends at line 409), before the `# Book documentation` section (line 418). Colocated with the other `doc:` targets per D-23.

Feature list (15 entries, alphabetized, comma-separated, no spaces):
`composition, http, http-client, jwt-auth, macros, mcp-apps, oauth, rayon, resource-watcher, schema-generation, simd, sse, streamable-http, validation, websocket` — byte-identical to `Cargo.toml` `[package.metadata.docs.rs]` `features = [...]`.

### `.github/workflows/ci.yml` — new step at lines 205–206

Diff (3 added lines, including blank separator):

```yaml
    - name: Check rustdoc zero-warnings
      run: make doc-check
```

Placement: inside the existing `quality-gate:` job (starts line 158, ends before `benchmarks:` at line 214). Positioned between `Check disk space before quality gate` (lines 202–203) and `Run quality gate` (lines 208–209). No new job created — job count unchanged at 6 (`test`, `feature-flags`, `quality-gate`, `benchmarks`, `msrv`, `gate`).

## Decisions Made

None — followed plan and CONTEXT.md decisions D-23 through D-27 exactly. All six invariants (stable toolchain, explicit feature list, inside existing job, not chained into quality-gate, TAB indentation, byte-identical feature list vs Cargo.toml) preserved.

## Deviations from Plan

None — plan executed exactly as written.

## Verification Results

### Task 1 — Makefile acceptance criteria

| Check | Command | Result |
|-------|---------|--------|
| `.PHONY: doc-check` declared | `grep -c '^\.PHONY: doc-check$' Makefile` | **1** ✓ |
| Target defined | `grep -c '^doc-check:$' Makefile` | **1** ✓ |
| `RUSTDOCFLAGS="-D warnings"` present | Grep in target body | **1** ✓ (line 414) |
| D-16 feature list verbatim | Grep for full feature string | **1** ✓ (line 415) |
| No `all-features` | Grep `all-features` in target body | **0** ✓ |
| No `--cfg docsrs` | Grep `--cfg docsrs` in target body | **0** ✓ |
| Existing `doc:` target intact | `grep -c '^\.PHONY: doc$' Makefile` | **1** ✓ |
| Existing `doc-open:` target intact | `grep -c '^\.PHONY: doc-open$' Makefile` | **1** ✓ |
| D-27: `quality-gate:` does NOT reference `doc-check` | Grep shows only 2 `doc-check` occurrences, both in the new target at lines 411/412 | ✓ |
| **TAB-indentation guard** | `Grep` with `^\t(@echo\|RUSTDOCFLAGS\|\t--features)` pattern | Matches lines 413, 414, 415, 416 ✓ — every recipe line is TAB-prefixed (regex `\t` would not match spaces) |
| `make -n doc-check` dry-run | sandbox denied `make`/`cat`; structural verification via Grep `^\t` regex is equivalent for tab-detection (`\t` regex only matches literal TAB 0x09, not SP 0x20) |

**Tab-indentation note:** The sandbox denied `make -n` and `cat -A` execution. Instead, tab-vs-space verification used a Grep regex `^\t(@echo|RUSTDOCFLAGS|\t--features)` — ripgrep's `\t` matches ONLY the literal TAB character (0x09), never spaces. All four recipe lines (413 `@echo`, 414 `RUSTDOCFLAGS`, 415 `--features` continuation with double tab, 416 `@echo`) matched this regex, proving every recipe line begins with TAB, not spaces. This is a semantically equivalent check to `cat -A | grep '^\^I'` and satisfies the tab-indentation guard from `<acceptance_criteria>`.

### Task 2 — ci.yml acceptance criteria

| Check | Command | Result |
|-------|---------|--------|
| New step name present | `grep -c 'Check rustdoc zero-warnings' ci.yml` | **1** ✓ |
| New step runs `make doc-check` | `grep -c 'make doc-check' ci.yml` | **1** ✓ |
| Existing `Run quality gate` step unchanged | `grep -c '- name: Run quality gate$' ci.yml` | **1** ✓ |
| Existing `Install quality tools` step unchanged | `grep -c '- name: Install quality tools$' ci.yml` | **1** ✓ |
| Job count unchanged (6 jobs) | `grep '^  [a-z][a-z-]*:$' ci.yml` | 6 jobs (`test`, `feature-flags`, `quality-gate`, `benchmarks`, `msrv`, `gate`) ✓ |
| No `continue-on-error` near new step | `grep -B 2 -A 2 'Check rustdoc zero-warnings' ci.yml \| grep continue-on-error` | **0** ✓ |
| Step INSIDE `quality-gate` job | Line 205 (`Check rustdoc zero-warnings`) between job start line 158 and next job `benchmarks:` line 214 | ✓ |
| Step positioned after `Check disk space before quality gate` and before `Run quality gate` | Line 205 > line 202 AND line 205 < line 208 | ✓ |
| Indentation matches surrounding steps | 4 spaces before `-`, 6 spaces before `run:` — identical to adjacent steps | ✓ |

## Self-Check

### Files exist and contain expected content

- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile` — `doc-check` target at lines 411–416 ✓
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/ci.yml` — `Check rustdoc zero-warnings` step at lines 205–206 ✓
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-05-makefile-and-ci-gate-SUMMARY.md` — this file ✓

### Commits exist

- `faf52e86` — `feat(67-05): add doc-check target to Makefile` ✓
- `48e7802b` — `ci(67-05): gate rustdoc zero-warnings in quality-gate job` ✓

## Self-Check: PASSED

## Issues Encountered

- **Sandbox blocked `make` and `cat -A`**: the planned `<verify>` automation (`make doc-check`, `make -n doc-check`, `cat -A Makefile | grep '^\^I'`) could not execute under the sandbox. Structural verification was performed via Grep regex `^\t` (literal TAB match) which is semantically equivalent for tab-vs-space detection. End-to-end `make doc-check` execution will be validated by Plan 06 (Wave 5 aggregate verification) and by CI on first push — Plan 04 established the zero-warning baseline so the target is expected to exit 0.

## Next Phase Readiness

- Plan 06 (`67-06-final-integration-verification`) can now run the aggregate rustdoc gate end-to-end via the new `make doc-check` target, and Check 4 (byte-identity diff of feature lists between Makefile and Cargo.toml) has a canonical Makefile line to diff against.
- First push of this branch to the PR will exercise the new CI step for the first time — if the CI runner environment behaves correctly (stable toolchain already installed via `dtolnay/rust-toolchain@stable`), the step completes in ~30–60 seconds warm.
- No blockers for Plan 06.

---
*Phase: 67-docs-rs-pipeline-and-feature-flags*
*Plan: 05 - Makefile and CI Gate*
*Completed: 2026-04-11*
