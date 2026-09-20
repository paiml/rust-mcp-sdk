---
phase: 90-openapi-built-in-server
plan: 09
subsystem: docs
tags: [openapi, docs, readme, pmcp-book, pmcp-course, cargo-pmcp, shape-a, shape-b, three-shapes]

# Dependency graph
requires:
  - phase: 90-openapi-built-in-server
    plan: 06
    provides: "pmcp-openapi-server Shape A binary — cli.rs (--config/--spec/--http), examples/openapi_server_min.rs (dispatch+build_server wiring), D-03 optional spec, D-02 one-engine"
  - phase: 90-openapi-built-in-server
    plan: 07
    provides: "london-tube parity fixture (tests/fixtures/london-tube.toml) — the canonical OpenAPI Shape-A worked example: api_key query-param auth + single-call + script tool"
  - phase: 90-openapi-built-in-server
    plan: 08
    provides: "cargo pmcp new --kind openapi-server scaffold (Shape B) — the cargo pmcp on-ramp the docs lead with; emits Cargo.toml/main.rs/config.toml/api.yaml/deploy.toml"
  - phase: 85-pure-config-binary
    provides: "pmcp-sql-server/README.md — the ANALOG README structure (improvement framing, Pareto, backends table, quickstart) mirrored for OpenAPI"
provides:
  - "crates/pmcp-openapi-server/README.md — Shape A binary docs (improvement framing, two-kind tools D-01, 6 auth variants D-05, --spec optionality D-03, CLI flags traced to cli.rs, cargo pmcp on-ramp)"
  - "pmcp-book/src/openapi-built-in-server.md — reference-depth book chapter (Ch 12.11), sibling of the SQL Ch 12.10"
  - "pmcp-course/src/openapi-built-in-server.md — guided walkthrough + exercise, sibling of the SQL ch08-5"
  - "SUMMARY.md toc entries in both mdbooks linking the new chapters next to the SQL built-in chapter"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Docs in three shapes (crate README + pmcp-book chapter + pmcp-course chapter) leading with the cargo pmcp on-ramp — the project's documented convention, mirrored from the SQL built-in docs"
    - "Doctest-safe fences only: every code fence is toml/bash/text-tagged (no bare ```rust that would run as a doctest); claims traced verbatim to cli.rs / http::auth::AuthConfig / the london-tube fixture; mdbook build is the toc/link gate (T-90-09-02)"
    - "Secrets shown only as ${ENV} references with a no-real-token discipline (T-90-09-01)"

key-files:
  created:
    - crates/pmcp-openapi-server/README.md
    - pmcp-book/src/openapi-built-in-server.md
    - pmcp-course/src/openapi-built-in-server.md
  modified:
    - pmcp-book/src/SUMMARY.md
    - pmcp-course/src/SUMMARY.md

key-decisions:
  - "The auth table documents SIX variants (none + 5 authenticated: api_key/bearer/basic/oauth2_client_credentials/oauth_passthrough), traced verbatim to AuthConfig in crates/pmcp-server-toolkit/src/http/auth.rs (the source has six). The plan prose listed the five authenticated ones; the README/chapters present none + 5 as the full set to match the real enum — a correctness alignment, not a deviation."
  - "Book chapter numbered Chapter 12.11 (immediately after the SQL Ch 12.10) and the course chapter placed right after the SQL config-driven chapter in Part III — keeping the two config-driven built-ins adjacent as siblings (memory feedback: Shape A binary vs Shape B scaffold are siblings)."
  - "Course chapter file lives at pmcp-course/src/openapi-built-in-server.md (top-level src, not under part3-deployment) per the plan's declared files_modified path; the toc link uses ./openapi-built-in-server.md and mdbook resolves it."

requirements-completed: [OAPI-09]

# Metrics
duration: 4min
completed: 2026-05-29
---

# Phase 90 Plan 09: OpenAPI Built-In Server Docs (Three Shapes) Summary

**Documented the OpenAPI built-in server in three shapes (OAPI-09): the `pmcp-openapi-server` crate README (Shape A binary), a `pmcp-book` reference chapter, and a `pmcp-course` guided walkthrough — each leading with the `cargo pmcp new --kind openapi-server` on-ramp, grounded verbatim in real source (CLI flags from `cli.rs`, the six-variant auth table from `http::auth::AuthConfig`, the two-kind tool model + london-tube worked example from the Plan 07 fixture), and doctest-safe (every fence `toml`/`bash`/`text`-tagged, no bare `rust`). Both mdbooks build clean with the new chapters wired into their `SUMMARY.md` toc next to the SQL built-in chapter.**

## Performance

- **Duration:** ~4 min
- **Started:** 2026-05-29T21:51:29Z
- **Completed:** 2026-05-29T21:55Z
- **Tasks:** 2
- **Files modified:** 5 (3 created, 2 modified)

## Accomplishments

- **Task 1 — crate README (`b8c5a061`):** `crates/pmcp-openapi-server/README.md` mirrors the `pmcp-sql-server` README structure for OpenAPI: the "improvement" framing (two inputs `config.toml` + optional `--spec`, no recompile; ~20% curated `[[tools]]` / ~80% Code Mode over the SAME HTTP engine), a Shape-A-vs-Shape-B sibling callout, the TWO-KIND tool model (single-call `path`/`method` AND script `script="""..."""`, D-01) with the london-tube single-call + script example, the SIX outgoing-auth variants table (`none` + `api_key`/`bearer`/`basic`/`oauth2_client_credentials`/`oauth_passthrough`, D-05) traced to `http::auth::AuthConfig`, `--spec` optionality (D-03 — curated-only boots without a spec; code-mode-without-spec warns and proceeds), a quickstart (build/install, minimal config, run), and a CLI-flags table (`--config` required / `--spec` optional / `--http` default `127.0.0.1:8080`) matching `cli.rs` exactly. Leads with the `cargo pmcp new --kind openapi-server` on-ramp.
- **Task 2 — book + course chapters + toc (`9af77f5a`):** `pmcp-book/src/openapi-built-in-server.md` (Chapter 12.11, reference depth: scaffold → run → customize → deploy, the Pareto diagram, two-kind tools, the six-variant auth table, D-03 optional spec, the deploy secret posture) and `pmcp-course/src/openapi-built-in-server.md` (guided walkthrough with prerequisites, a labeled ASCII architecture diagram, a "two kinds of tools" teaching section, a when-to-use table, and a ship-a-two-tool exercise). Both lead with the `cargo pmcp` on-ramp, position the OpenAPI built-in as the HTTP sibling of the SQL built-in (cross-linked to Ch 12.10 / the SQL course chapter), and use only `${ENV}` secret references. Both `SUMMARY.md` files gained a toc entry next to the SQL built-in chapter. `mdbook build pmcp-book` and `mdbook build pmcp-course` both exit 0.

## Task Commits

1. **Task 1: crate README for pmcp-openapi-server (Shape A binary)** — `b8c5a061` (docs)
2. **Task 2: pmcp-book + pmcp-course OpenAPI built-in chapters + toc** — `9af77f5a` (docs)

## Files Created/Modified

- `crates/pmcp-openapi-server/README.md` (created) — Shape A binary docs: improvement framing, two-kind tools (D-01), six auth variants (D-05), `--spec` optionality (D-03), CLI flags table traced to `cli.rs`, cargo pmcp on-ramp.
- `pmcp-book/src/openapi-built-in-server.md` (created) — Chapter 12.11 reference chapter.
- `pmcp-course/src/openapi-built-in-server.md` (created) — guided walkthrough + exercise.
- `pmcp-book/src/SUMMARY.md` (modified) — toc entry after Ch 12.10.
- `pmcp-course/src/SUMMARY.md` (modified) — toc entry after the SQL config-driven chapter.

## Decisions Made

- **Six auth variants (not five) in the table.** The plan prose listed the five authenticated variants (`api_key`/`bearer`/`basic`/`oauth2_client_credentials`/`oauth_passthrough`); the real `AuthConfig` enum (`crates/pmcp-server-toolkit/src/http/auth.rs`) has SIX — `None` plus those five. The README and both chapters document all six (`none` is the default) to match the source verbatim (T-90-09-02 correctness). This is alignment to the real surface, not a scope change.
- **Chapter placement keeps the two built-ins adjacent.** Book → Chapter 12.11 right after the SQL Ch 12.10; course → right after the SQL config-driven chapter in Part III. The OpenAPI built-in is explicitly framed as the HTTP sibling of the SQL built-in, with cross-links both ways.
- **No bare `rust` fences.** Every fence is `toml`/`bash`/`text` so nothing runs as a doctest and the README adds no compile surface; CLI/config/auth claims are traced to `cli.rs` / `AuthConfig` / the london-tube fixture rather than invented.

## Deviations from Plan

None — plan executed as written. (The six-vs-five auth-variant count is a source-alignment decision documented above, not a deviation: it reports what `AuthConfig` actually exposes.)

## Issues Encountered

None. Both mdbooks were already buildable; the new chapters and toc entries resolved with no broken-link or toc errors.

## TDD Gate Compliance

This plan's frontmatter is `type: execute` (no per-task `tdd="true"`). It is a docs-only plan — the verification gate is the `mdbook build` for both books plus the acceptance greps (files exist, toc links present, `openapi-server` substring present), all of which passed. No code under test.

## Known Stubs

None. The README, both chapters, and both toc entries are complete and substantive. All examples are grounded in real source (the london-tube fixture, `cli.rs`, `AuthConfig`); no placeholder or "coming soon" text.

## Threat Flags

None — docs only, no new network endpoint / auth path / file-access surface. The plan's threat register is satisfied: T-90-09-01 (no real secret in any fence — every secret is a `${ENV}` reference) and T-90-09-02 (no CLI/config drift — flags traced to `cli.rs`, auth to `AuthConfig`, fences doctest-safe, mdbook build gate green).

## Self-Check: PASSED

- All 3 created files present on disk: `crates/pmcp-openapi-server/README.md`, `pmcp-book/src/openapi-built-in-server.md`, `pmcp-course/src/openapi-built-in-server.md`; both modified `SUMMARY.md` files updated.
- Both task commits present in git history: `b8c5a061` (Task 1), `9af77f5a` (Task 2).
- Acceptance: README `test -f` + `grep openapi-server` OK; CLI flags in README = `--config`/`--spec`/`--http` (match `cli.rs`, no invented flags); `grep -c openapi-built-in-server` = 1 in each SUMMARY; both chapters contain `openapi-server`; `mdbook build pmcp-book` and `mdbook build pmcp-course` both exit 0; no bare `rust` fences; no file deletions in the two task commits.

---
*Phase: 90-openapi-built-in-server*
*Completed: 2026-05-29*
