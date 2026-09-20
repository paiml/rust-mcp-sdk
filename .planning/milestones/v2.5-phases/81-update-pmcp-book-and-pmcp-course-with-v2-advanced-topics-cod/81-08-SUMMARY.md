---
phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod
plan: 08
subsystem: docs
tags: [docs, gap-closure, version-pin, audit-e, code-mode]

# Dependency graph
requires:
  - phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod
    provides: "81-07-AUDIT.md FAIL findings #6 and #7 (book/course pin byte-inequality)"
provides:
  - "Book Code Mode chapter dependency pins byte-equal to course chapter pins"
  - "Audit E (version-pin consistency) gap closed for plan 81-10 re-audit"
affects: [81-10-re-audit, future-pmcp-code-mode-version-bumps]

# Tech tracking
tech-stack:
  added: []
  patterns: ["byte-equality audit invariant for cross-document version pins"]

key-files:
  created: []
  modified:
    - pmcp-book/src/ch12-9-code-mode.md

key-decisions:
  - "Adopted course's exact-version pins (\"0.5.1\", \"0.2.0\") rather than relaxing course to semver-range pins, because the authoritative crates/pmcp-code-mode{,-derive}/Cargo.toml strings are exact versions and adopting them makes the book strictly more precise."

patterns-established:
  - "Cross-document pin alignment: when an audit demands byte-equality between two documents that pin a workspace crate, choose the pin string that matches the authoritative Cargo.toml `version = \"...\"` line."

requirements-completed: []

# Metrics
duration: 1min
completed: 2026-05-15
---

# Phase 81 Plan 08: Align Book Code Mode Dependency Pins to Course Summary

**Two-line edit aligning `pmcp-book/src/ch12-9-code-mode.md` L89/L90 dependency pins to byte-equal the course's `pmcp-code-mode = "0.5.1"` / `pmcp-code-mode-derive = "0.2.0"` exact-version pins, closing Audit E FAIL findings #6 and #7.**

## Performance

- **Duration:** ~1 min
- **Started:** 2026-05-15T23:32:39Z
- **Completed:** 2026-05-15T23:33:32Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Audit E FAIL findings #6 and #7 (book vs. course version-pin byte-inequality) closed.
- Book Code Mode chapter dependency pins now byte-equal both the course chapter pins and the authoritative workspace `Cargo.toml` version strings.
- `mdbook build` exits 0 — no rendering regression introduced.
- Diff scope minimized to exactly +2/-2 lines in a single file, no surrounding pedagogy disturbed.

## Task Commits

Each task was committed atomically:

1. **Task 1: Align book Code Mode chapter version pins to course (Audit E findings #6, #7)** — `dd4c3e3a` (docs)

## Files Created/Modified

- `pmcp-book/src/ch12-9-code-mode.md` — L89 abbreviated pin `pmcp-code-mode = "0.5"` upgraded to exact pin `pmcp-code-mode = "0.5.1"`; L90 abbreviated pin `pmcp-code-mode-derive = "0.2"` upgraded to exact pin `pmcp-code-mode-derive = "0.2.0"`. No other lines touched.

## Before / After Pin Values

| Line | Before                              | After                                |
| ---- | ----------------------------------- | ------------------------------------ |
| 89   | `pmcp-code-mode = "0.5"`            | `pmcp-code-mode = "0.5.1"`           |
| 90   | `pmcp-code-mode-derive = "0.2"`     | `pmcp-code-mode-derive = "0.2.0"`    |

### Byte-Equality Confirmation

| Source                                                      | `pmcp-code-mode` pin string             | `pmcp-code-mode-derive` pin string             |
| ----------------------------------------------------------- | --------------------------------------- | ---------------------------------------------- |
| `pmcp-book/src/ch12-9-code-mode.md` (L89/L90, post-edit)    | `pmcp-code-mode = "0.5.1"`              | `pmcp-code-mode-derive = "0.2.0"`              |
| `pmcp-course/src/part8-advanced/ch22-code-mode.md` (L67/L68) | `pmcp-code-mode = "0.5.1"`              | `pmcp-code-mode-derive = "0.2.0"`              |
| `crates/pmcp-code-mode/Cargo.toml` (`version` line)          | `version = "0.5.1"` (authoritative)     | —                                              |
| `crates/pmcp-code-mode-derive/Cargo.toml` (`version` line)   | —                                       | `version = "0.2.0"` (authoritative)            |

All three sources are now byte-equal at the relevant pin strings, per the plan's automated verification check (`AUDIT E PASS: book/course pins byte-equal AND match authoritative Cargo.toml`).

## Decisions Made

- Followed plan as specified — adopted exact-version pins from the course (matches authoritative Cargo.toml strings). No deviation needed.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Closure Target

This plan closes the following findings from `.planning/phases/81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod/81-07-AUDIT.md`:

- **Audit E FAIL finding #6** — book `pmcp-code-mode` pin byte-inequality vs. course.
- **Audit E FAIL finding #7** — book `pmcp-code-mode-derive` pin byte-inequality vs. course.

## Verdict

`Audit E gap closed — book and course pin lines byte-equal.`

## Next Phase Readiness

- **Plan 81-10 (re-audit)** can re-run Audit E and observe the two findings transition FAIL → PASS without further pin edits.
- No follow-on work required from this plan; the cross-document byte-equality invariant will hold until any future bump of `pmcp-code-mode` or `pmcp-code-mode-derive`. At that point both the book L89/L90 pins, the course L67/L68 pins, and the workspace `Cargo.toml` `version` strings must be bumped together (single atomic commit) to preserve the invariant.

## Self-Check: PASSED

- `FOUND: .planning/phases/81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod/81-08-SUMMARY.md`
- `FOUND: pmcp-book/src/ch12-9-code-mode.md`
- `FOUND: commit dd4c3e3a` (Task 1: docs(81-08): align book Code Mode dep pins to course)

---
*Phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod*
*Completed: 2026-05-15*
