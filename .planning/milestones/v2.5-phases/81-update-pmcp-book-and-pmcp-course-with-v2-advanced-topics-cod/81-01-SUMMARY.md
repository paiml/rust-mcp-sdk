---
phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod
plan: 01
subsystem: docs / book
tags: [docs, book, skills, sep-2640]
requires: []
provides:
  - "Book Skills chapter (ch12-8) with dual-surface invariant up front, three-tier walkthrough, runnable doctest"
  - "SUMMARY.md index entry slotting Skills between Tasks (ch12-7) and Code Mode (ch12-9)"
  - "Compile-verified rust,no_run doctest in src/server/skills.rs byte-equal to the chapter doctest"
affects:
  - "pmcp-book/ render — new chapter appears in Part III: Advanced Features"
  - "cargo test --doc -p pmcp --features skills,full now covers the dual-surface bootstrap path"
tech-stack:
  added: []
  patterns:
    - "Inline-excerpt + GitHub cross-link (mirrors ch12-7-tasks.md structure)"
    - "Grep-stable boundaries on every inline excerpt (whole functions / whole SKILL.md bodies)"
    - "Compile-verified doctest as drift-prevention (Phase 66 pmcp-macros pattern)"
key-files:
  created:
    - pmcp-book/src/ch12-8-skills.md
    - .planning/phases/81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod/81-01-SUMMARY.md
    - /tmp/81-01-doctest-snippet.txt
  modified:
    - pmcp-book/src/SUMMARY.md
    - src/server/skills.rs
decisions:
  - "Compile-probe-first ordering (Phase 81 R-4): the doctest body was probe-validated under --features skills,full before being locked into source. The candidate body from the plan compiled cleanly on first try, confirming the public API surface (Server::builder().name/version/skill/bootstrap_skill_and_prompt/build) is stable."
  - "Doctest placement: module-level //! comment in src/server/skills.rs (option (a) per plan). Skills.rs already had a structured 4-paragraph module doc; appended a labeled '# Book Chapter Doctest' section so the original prose is unchanged."
  - "Chapter format: rust,ignore for pedagogical inline excerpts; rust,no_run for the single doctest at the end. Synthetic JSON snippet preceded by <!-- synthetic --> per the plan's R-7 / Audit A escape-hatch convention."
  - "Skipped the full make quality-gate (Phase 81 R-7): focused cargo fmt --all -- --check + cargo clippy -p pmcp --features skills,full --no-deps -- -D warnings exited 0. The pre-commit hook on the developer's branch still runs the full gate at commit time."
metrics:
  duration_min: 9
  duration_sec: 571
  completed: 2026-05-15T20:18:08Z
  tasks_total: 3
  tasks_completed: 3
  files_changed: 3
  commits: 3
---

# Phase 81 Plan 01: Add Skills (SEP-2640) Book Chapter Summary

## One-Liner

Adds the missing PMCP book chapter for SEP-2640 Skills, slots it between Tasks (ch12-7) and Code Mode (ch12-9), and embeds a compile-verified `rust,no_run` doctest in `src/server/skills.rs` byte-equal to the chapter doctest.

## What Was Built

Plan 81-01 closed the documentation gap left after Phase 80 shipped SEP-2640 Skills support. Three artifacts:

1. **`pmcp-book/src/ch12-8-skills.md`** (401 lines, new file). Six mandatory H2 sections in the required order: `## The Dual-Surface Invariant`, `## Tier 1: Hello-World Skill`, `## Tier 2: Refunds Skill with References (SEP-2640 §9 Visibility Filtering)`, `## Tier 3: Code-Mode Skill (Composition with Another Advanced Feature)`, `## Cross-SDK Compatibility (Why Three Tiers Match Other Reference Implementations)`, `## Future Work (Deferred from Phase 80)`. The dual-surface invariant lead appears at line 28 (well within the first 100 lines). Inline excerpts pull from `examples/s44_server_skills.rs`, `examples/c10_client_skills.rs`, the three `examples/skills/*/SKILL.md` bodies, and the `as_prompt_text()` implementation in `src/server/skills.rs` — every excerpt uses grep-stable function/section boundaries rather than line-range citations.

2. **`pmcp-book/src/SUMMARY.md`** (one line inserted). New entry `- [Chapter 12.8: Skills — Agent Workflow Instructions](ch12-8-skills.md)` slotted between the existing `ch12-7-tasks.md` (line 34) and `ch12-9-code-mode.md` (line 36) entries. No other lines modified.

3. **`src/server/skills.rs`** (one doctest block appended to the module-level `//!` doc comment). The `rust,no_run` block is byte-identical to both `/tmp/81-01-doctest-snippet.txt` (locked snippet) and the `rust,no_run` block in `ch12-8-skills.md` (verified via `diff`). The doctest runs under `cargo test --doc -p pmcp --features skills,full` and is skipped (does not fail to compile) under the no-features build because the surrounding `src/server/skills.rs` module is gated by `#[cfg(feature = "skills")]`.

## Path Inventory

| Artifact | Path | Status |
|----------|------|--------|
| Book chapter | `pmcp-book/src/ch12-8-skills.md` | Created (401 lines) |
| Book index | `pmcp-book/src/SUMMARY.md` | Modified (+1 line) |
| Source doctest | `src/server/skills.rs` | Modified (+26 lines; doctest in module-level //! comment, anchor: `# Book Chapter Doctest` section grep-anchor immediately after the existing 4-paragraph module doc, immediately before the `use std::collections::HashMap;` import line) |
| Locked snippet | `/tmp/81-01-doctest-snippet.txt` | Created (transient discovery output) |

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | `82965de2` | `docs(81-01): add ch12-8 Skills chapter (SEP-2640)` |
| 2 | `32cd4e00` | `docs(81-01): slot ch12-8-skills.md into pmcp-book SUMMARY index` |
| 3 | `a8a853f7` | `test(81-01): embed compile-verified Skills doctest in src/server/skills.rs` |

## Verification

### Task 1: Chapter file structure (`pmcp-book/src/ch12-8-skills.md`)

- `test -f`: PASS
- All six mandatory H2 sections present: PASS
- `## The Dual-Surface Invariant` appears at line 28 (within first 100 lines): PASS
- Exactly one `rust,no_run` block (chapter doctest): PASS
- Contains the substring `byte-equal` (R-2 cross-property anchor for 81-07 Audit F): PASS
- Cites `tests/skills_integration.rs` by full path (R-2 cross-property anchor): PASS
- Line count 401 ≥ 400 minimum: PASS

### Task 2: SUMMARY index slot

- `ch12-8-skills.md` appears between `ch12-7-tasks.md` and `ch12-9-code-mode.md`: PASS
- Single new line inserted; no other lines modified: PASS
- Em-dash style matches surrounding entries: PASS

### Task 3: Compile-verified doctest

- Compile-probe ran first (Phase 81 R-4 ordering). The candidate body from the plan compiled cleanly under `cargo test --doc -p pmcp --features skills,full` on first try. No body adjustments required (Server::builder API confirmed stable against the prescribed shape).
- `/tmp/81-01-doctest-snippet.txt`: created.
- Doctest body in `src/server/skills.rs` byte-equal to `/tmp/81-01-doctest-snippet.txt`: PASS (`diff` empty).
- Doctest body in `pmcp-book/src/ch12-8-skills.md` (the `rust,no_run` block) byte-equal to `/tmp/81-01-doctest-snippet.txt`: PASS (`diff` empty).
- `cargo test --doc -p pmcp --features skills,full`: PASS (364 doctests passed, 78 ignored, 0 failures, ~63s).
- `cargo build -p pmcp` (no features): PASS (the doctest does not break the no-features build because the surrounding module is `#[cfg(feature = "skills")]`).
- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy -p pmcp --features skills,full --no-deps -- -D warnings`: exit 0 ("No issues found").

### Probe Outcome (Phase 81 R-4)

The discovery sub-step ran BEFORE the snippet was locked into source. The candidate body from the plan compiled cleanly under `--features skills,full` on first try, so no adjustments were required. The body that finally compiled IS the locked snippet at `/tmp/81-01-doctest-snippet.txt`. This confirms that:

- `Server::builder().build()` returns `Result<Server, _>` (the `?` operator works against `Box<dyn Error>`).
- `Skill::as_prompt_text()` is publicly callable from outside the crate.
- Both `.skill(...)` and `.bootstrap_skill_and_prompt(...)` are reachable from `pmcp::Server::builder()` (the public path, not just `ServerCoreBuilder`).

### Quality Gate Posture (R-7)

Per the plan's Phase 81 R-7 directive, the full `make quality-gate` was NOT run as part of this plan's verify — it remains the pre-commit hook's responsibility at the developer's branch commit time. Focused gates run:

- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy -p pmcp --features skills,full --no-deps -- -D warnings`: exit 0.

Both gates exited 0 with no findings.

## Deviations from Plan

None — plan executed exactly as written. The compile-probe (R-4) succeeded on first try with no body adjustments, so the locked snippet matches the plan's fallback candidate verbatim.

## Threat Surface Scan

Documentation-only changes plus one doctest addition to an existing public surface. No new authentication, authorization, input validation, or trust-boundary surface introduced. The plan's threat model (T-81-01-01..04) is fully addressed:

- T-81-01-01 (misleading inline excerpts): all excerpts use grep-stable boundaries; 81-07 can audit them.
- T-81-01-02 (misleading security framing): chapter does not claim Skills grant authority; Tier 3 cross-links to Chapter 12.9 for the actual execution-time authorization surface (`validate_code` / `execute_code` tools).
- T-81-01-03 (misleading dual-surface explanation): chapter cites `tests/skills_integration.rs` so readers can verify the invariant runtime-enforcement claim.
- T-81-01-04 (fragile doctest, R-4): the compile-probe ran first and succeeded; doctest is locked at a known-compiling body.

## Self-Check: PASSED

Verified post-write:
- FOUND: `pmcp-book/src/ch12-8-skills.md` (401 lines)
- FOUND: chapter sections (all 6 H2)
- FOUND: SUMMARY.md `ch12-8-skills.md` entry between ch12-7 and ch12-9
- FOUND: skills.rs doctest body byte-equal to locked snippet and chapter
- FOUND: commits `82965de2`, `32cd4e00`, `a8a853f7` in `git log --oneline`
- FOUND: `cargo test --doc -p pmcp --features skills,full` exits 0 with 364 passed
