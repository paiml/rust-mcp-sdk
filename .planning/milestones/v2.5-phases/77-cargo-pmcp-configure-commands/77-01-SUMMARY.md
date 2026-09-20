---
phase: 77
plan: 01
subsystem: planning-foundation
tags: [requirements, version-bump, changelog, scaffolding]
dependency-graph:
  requires: []
  provides:
    - REQ-77-01..REQ-77-11 (minted in REQUIREMENTS.md)
    - cargo-pmcp 0.11.0 version line (consumed by Plans 02..09)
    - CHANGELOG [0.11.0] stub (prepended for Phase 77 entries)
  affects:
    - .planning/REQUIREMENTS.md
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/CHANGELOG.md
tech-stack:
  added: []
  patterns:
    - REQ-NN-XX traceability rows + coverage counter pattern (matches CLI-AUTH/SDK-DCR seeding)
    - CHANGELOG stub with placeholder date (2026-04-XX) finalized later by Plan 09
key-files:
  created: []
  modified:
    - .planning/REQUIREMENTS.md
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/CHANGELOG.md
decisions:
  - "REQ entries reference Phase 77 RESEARCH/VALIDATION/CONTEXT verbatim — text matches plan body so downstream plans can grep-extract the behavior contract"
  - "CHANGELOG date stub = `2026-04-XX` per plan instruction — actual date filled by Plan 09 when phase ships"
  - "No new Cargo.toml dependencies — regex/dirs/tempfile/toml/proptest/serde all already present per 77-RESEARCH Environment Availability table"
metrics:
  duration: 2m
  completed: 2026-04-26
  tasks_completed: 2
  files_modified: 3
---

# Phase 77 Plan 01: REQ Minting + cargo-pmcp 0.11.0 Bump Summary

Minted REQ-77-01..REQ-77-11 in REQUIREMENTS.md with traceability rows, and bumped `cargo-pmcp` from 0.10.0 → 0.11.0 with a Phase 77 CHANGELOG stub — pure scaffolding so every downstream Phase 77 plan can reference these REQ-IDs and the version-check on first task is consistent.

## Tasks Completed

| Task | Name                                                        | Commit     | Files                                                                       |
| ---- | ----------------------------------------------------------- | ---------- | --------------------------------------------------------------------------- |
| 1    | Mint REQ-77-01..REQ-77-11 in REQUIREMENTS.md                | `13cc0821` | `.planning/REQUIREMENTS.md`                                                 |
| 2    | Bump cargo-pmcp to 0.11.0 + CHANGELOG stub                  | `36ac194f` | `cargo-pmcp/Cargo.toml`, `cargo-pmcp/CHANGELOG.md`                          |

## Outcome vs Plan

### REQ-77-01..REQ-77-11 (Task 1)

Inserted `### CLI configure subcommand (Phase 77)` heading after the `### CLI auth subcommand + SDK DCR (Phase 74)` block in REQUIREMENTS.md, with all 11 REQ entries verbatim from the plan:

- **REQ-77-01**: `cargo pmcp configure {add,use,list,show}` subcommand group
- **REQ-77-02**: `~/.pmcp/config.toml` typed-enum schema with `deny_unknown_fields`
- **REQ-77-03**: `.pmcp/active-target` workspace marker (single-line, permissive read / strict write)
- **REQ-77-04**: `PMCP_TARGET=<name>` env override + global `--target <name>` flag
- **REQ-77-05**: stderr header banner with fixed field ordering (api_url / aws_profile / region / source)
- **REQ-77-06**: field-level precedence `ENV > --flag > active target > deploy.toml`
- **REQ-77-07**: raw-credential pattern rejection (AKIA, ASIA, ghp_, github_pat_, sk_live_, AIza)
- **REQ-77-08**: atomic config writes via `tempfile::NamedTempFile::persist` with Unix 0o600/0o700 modes
- **REQ-77-09**: byte-identical Phase 76 behavior when `~/.pmcp/config.toml` is absent
- **REQ-77-10**: ALWAYS gates pass (fuzz / property / unit / cargo run --example)
- **REQ-77-11**: banner integration across all enumerated target-consuming entry points (HIGH-2 from 77-REVIEWS)

11 traceability rows added after the `CLI-AUTH-01 | Phase 74 | Complete` row. Coverage block updated `31 total → 42 total` (`+11 seeded by Phase 77`). Footer line appended: `*Last updated: 2026-04-26 — added 11 REQ-77-* IDs seeded by Phase 77 cargo pmcp configure commands research.*`

**Acceptance grep counts (all pass):**
- `grep -c "^- \[ \] \*\*REQ-77-" .planning/REQUIREMENTS.md` = 11 ✓
- `grep -c "^| REQ-77-" .planning/REQUIREMENTS.md` = 11 ✓
- `grep "REQ-77-01.*Phase 77.*Pending" .planning/REQUIREMENTS.md` = 1 line ✓
- `grep -q "### CLI configure subcommand (Phase 77)" .planning/REQUIREMENTS.md` exits 0 ✓
- `grep -q "v2.1 requirements: 42 total" .planning/REQUIREMENTS.md` exits 0 ✓

### cargo-pmcp 0.11.0 + CHANGELOG (Task 2)

`cargo-pmcp/Cargo.toml` line 3: `version = "0.10.0"` → `version = "0.11.0"`. Touched nothing else in the manifest (no new dependencies added — regex/dirs/tempfile/toml/proptest/serde all already present).

`cargo-pmcp/CHANGELOG.md` prepended `## [0.11.0] - 2026-04-XX` section directly above the existing `## [0.10.0]` entry, covering:
- **Added** — configure command group + `--target <name>` global flag + stderr banner + `pmcp_config_toml_parser` fuzz target + property tests + `multi_target_monorepo` example
- **Changed** — `--target <type>` → `--target-type <type>` rename with `#[arg(alias = "target")]` deprecation for one release cycle (removed in 0.12.0); bare `--target <name>` now refers to a NAMED target from `~/.pmcp/config.toml`
- **Security** — raw-credential pattern rejection at insertion time + atomic writes (0o600/0o700)

Date placeholder `2026-04-XX` per plan instruction — Plan 09 fills the actual day.

**Acceptance counts (all pass):**
- `grep -c '^version = "0.11.0"$' cargo-pmcp/Cargo.toml` = 1 ✓
- `grep -c '^version = "0.10.0"$' cargo-pmcp/Cargo.toml` = 0 ✓
- `grep -c '^## \[0.11.0\]' cargo-pmcp/CHANGELOG.md` = 1 ✓
- `grep -q "DEPRECATED.*--target" cargo-pmcp/CHANGELOG.md` exits 0 ✓
- `cd cargo-pmcp && cargo build --quiet` exits 0 (pre-existing dead-code warnings in pentest module unrelated to this change) ✓
- `git diff cargo-pmcp/Cargo.toml | grep -c '^[-+]version'` = 2 ✓

## Deviations from Plan

None — plan executed exactly as written.

## Decisions Made

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Insert REQ-77 section after Phase 74 (CLI auth), before Future Requirements | Matches existing seeding pattern (Phase 69 / 72 / 72.1 / 74 all seeded as `###` subsections under v2.1) | Clean append; no intra-file reflow needed |
| 11 REQ-IDs (not 10) | 77-VALIDATION.md explicitly adds REQ-77-11 (banner integration sites) per HIGH-2 fix from 77-REVIEWS — already merged into upstream plan body in commit f3611f08 | Plan body line 81 explicitly references REQ-77-11 |
| Coverage incremented `31 → 42` (+11) | Matches the +11 IDs seeded; subdivision in coverage line cites `+11 seeded by Phase 77` | Counter stays auditable |
| `2026-04-XX` literal date in CHANGELOG | Per plan body line 141; actual release day filled by Plan 09 | No date drift mid-phase |

## Verification

| Check                                                                     | Status |
| ------------------------------------------------------------------------- | ------ |
| 11 REQ-77-XX entries with traceability rows in REQUIREMENTS.md            | ✓      |
| cargo-pmcp/Cargo.toml version is 0.11.0                                   | ✓      |
| CHANGELOG has `[0.11.0] - 2026-04-XX` covering Added/Changed/Security     | ✓      |
| `cargo build` exits 0 at new version                                      | ✓      |
| `git diff cargo-pmcp/Cargo.toml | grep -c '^[-+]version'` = 2 (one removed, one added) | ✓ |

## Threat Surface Scan

No new security-relevant surface introduced — pure metadata/documentation changes. Threat register entries T-77-12-A (Cargo.toml accidental dep injection) and T-77-12-B (CHANGELOG leak) both `accept` per plan threat model and verified compliant:
- T-77-12-A: `git diff cargo-pmcp/Cargo.toml` shows ONLY the version line changed (2 `^[-+]version` matches, 0 `^[-+]\w+ =` dep lines).
- T-77-12-B: CHANGELOG entries are intentional public artifacts; `2026-04-XX` placeholder used until release (per plan).

## Self-Check: PASSED

**Files verified:**
- `[ -f .planning/REQUIREMENTS.md ]` → FOUND
- `[ -f cargo-pmcp/Cargo.toml ]` → FOUND
- `[ -f cargo-pmcp/CHANGELOG.md ]` → FOUND
- `[ -f .planning/phases/77-cargo-pmcp-configure-commands/77-01-SUMMARY.md ]` → FOUND (this file)

**Commits verified in `git log --oneline`:**
- `13cc0821` (Task 1) → FOUND
- `36ac194f` (Task 2) → FOUND
