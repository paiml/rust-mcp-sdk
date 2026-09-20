---
phase: 84-sql-connectors-postgres-mysql-athena-sqlite
plan: 08
subsystem: testing
tags: [fuzz, libfuzzer, corpus, toml, config-parser, release-process, requirements-closeout]

# Dependency graph
requires:
  - phase: 84-01..07
    provides: SqlConnector trait, Dialect enum, translate_placeholders + build_code_mode_prompt, per-backend connector crates, SQLite feature, [database].url field
  - phase: 77
    provides: pmcp_server_toolkit_config_parser fuzz target + corpus pattern (reused, not duplicated — D-14)
provides:
  - 3 per-backend fuzz corpus seeds (postgres/mysql/athena) for the config-parser target
  - 4 REVIEWS M6 adversarial URL corpus seeds (extremely-long >10KB, non-ASCII, malformed env-ref, SQL-injection-shape)
  - fuzz_corpus_seeds_parse_or_explicitly_fail smoke test (well-formed seeds parse; adversarial tolerated as Err; no panic)
  - CLAUDE.md publish-order updated with the three per-backend connector crates
  - REQUIREMENTS.md CONN-01..08 + TEST-01 + TEST-07 formally closed for Phase 84
  - 84-VALIDATION.md flipped to nyquist_compliant + status complete with filled per-task matrix
affects: [phase-85, phase-86, phase-88, release-process, fuzz-ci]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Extend the single existing fuzz target's corpus (D-14) — never duplicate the target for a new schema field"
    - "Adversarial corpus seeds are well-formed TOML so libfuzzer mutates from a valid base rather than bouncing off TOML parse errors"
    - "Seed-parse smoke test pins well-formed seeds (MUST parse) while tolerating adversarial seeds as Err — no-panic is the only universal invariant"
    - "Phase-scoped verification sweep when a pre-existing unrelated lint blocks the broad workspace gate (documented, not fixed)"

key-files:
  created:
    - crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-postgres-backend.toml
    - crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-mysql-backend.toml
    - crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-athena-backend.toml
    - crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-url-extremely-long.toml
    - crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-url-non-ascii.toml
    - crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-url-malformed-env-ref.toml
    - crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-url-sql-injection-shape.toml
  modified:
    - crates/pmcp-server-toolkit/tests/reference_configs.rs
    - CLAUDE.md
    - .planning/REQUIREMENTS.md
    - .planning/phases/84-sql-connectors-postgres-mysql-athena-sqlite/84-VALIDATION.md

key-decisions:
  - "Staged only the 7 named seed files + the test file; left the 5,579 libfuzzer hash-named corpus entries untracked/preserved (Phase 77/79 convention — only named seed-*.toml are tracked)"
  - "aws-sdk-glue grep guard interpreted by intent (no Glue in the build graph), not literally — the two matches are intentional 'NO aws-sdk-glue' documentation comments kept per Wave 0 decision; cargo tree confirms zero Glue dependency"
  - "Verification sweep scoped to Phase 84's 4 crates — the broad make quality-gate is blocked by pre-existing unrelated rust-1.95.0 pedantic lints in pmcp-widget-utils (deferred-items.md), deferred to CI, NOT fixed"
  - "Corrected CONN-07/TEST-01 Athena descriptions in REQUIREMENTS.md from 'Glue catalog' to 'GetTableMetadata' to match shipped reality (Landmine #4 / D-08)"

patterns-established:
  - "D-14 corpus-extension: a single config-parser fuzz target covers any new ServerConfig field via deny_unknown_fields; add seeds, don't add targets"
  - "REVIEWS M6 adversarial-URL seed family as the template for stress-seeding new free-text config fields"

requirements-completed: [TEST-07, TEST-01, CONN-01, CONN-02, CONN-03, CONN-04, CONN-05, CONN-06, CONN-07, CONN-08]

# Metrics
duration: 24min
completed: 2026-05-26
---

# Phase 84 Plan 08: Wave 3 Closeout Summary

**Extended the Phase 77 config-parser fuzz corpus with 3 per-backend + 4 REVIEWS M6 adversarial URL seeds (60s / 1.19M-run fuzz clean), slotted the three connector crates into CLAUDE.md publish-order, closed the CONN/TEST requirements ledger, and flipped 84-VALIDATION.md to nyquist-compliant — finalizing Phase 84.**

## Performance

- **Duration:** ~24 min
- **Started:** 2026-05-26 (post 84-07)
- **Completed:** 2026-05-26
- **Tasks:** 2
- **Files modified:** 11 (7 created seeds + 1 test + 3 planning/docs)

## Accomplishments

- Added 3 per-backend (postgres/mysql/athena) + 4 REVIEWS M6 adversarial URL corpus seeds to the single existing `pmcp_server_toolkit_config_parser` fuzz target (D-14 extend-don't-duplicate). The extremely-long seed is 12,221 bytes (≥10KB requirement).
- Ran the fuzz target 60s against the extended corpus on local nightly: **1,194,703 runs, no crash, exit 0**.
- Added `fuzz_corpus_seeds_parse_or_explicitly_fail` to `reference_configs.rs`: well-formed seeds MUST parse via `ServerConfig::from_toml`; the 4 adversarial seeds are tolerated as `Err` (no-panic is the universal invariant). Test green.
- Updated CLAUDE.md publish-order: `pmcp-toolkit-postgres`/`-mysql`/`-athena` inserted as slots 6-8 between `pmcp-server-toolkit` (5) and `mcp-tester` (9).
- Closed REQUIREMENTS.md: CONN-01..08 + TEST-01 + TEST-07 confirmed `[x]`/Complete; corrected Athena descriptions; appended Phase 84 closure footer.
- Flipped 84-VALIDATION.md to `nyquist_compliant: true` + `status: complete` + `wave_0_complete: true`; filled the per-task matrix with real plan/task IDs (all 12 rows ✅ green); signed approval.
- Ran the full REVIEWS regression-guard sweep (H1–H7 + M1–M6): all pass.

## Task Commits

1. **Task 1: Seed fuzz corpus + seed-parse smoke test + 60s fuzz** - `84dcddac` (test)
2. **Task 2: Verification sweep + CLAUDE.md publish-order + REQUIREMENTS closure + VALIDATION flip** - `0f4d8db2` (docs)

**Plan metadata:** (final docs commit — this SUMMARY + STATE.md + ROADMAP.md)

## Files Created/Modified

- `fuzz/corpus/.../seed-postgres-backend.toml` - minimal Postgres `[database]` seed (`type="postgres"`, `env:DATABASE_URL`, 2 tables)
- `fuzz/corpus/.../seed-mysql-backend.toml` - minimal MySQL seed with `[database.pool]`
- `fuzz/corpus/.../seed-athena-backend.toml` - Athena seed mirroring open-images shape (output_location, workgroup)
- `fuzz/corpus/.../seed-url-extremely-long.toml` - 12,221-byte URL (>10KB) stressing bounded-string handling
- `fuzz/corpus/.../seed-url-non-ascii.toml` - unicode in URL components (`用户:密码@主机`)
- `fuzz/corpus/.../seed-url-malformed-env-ref.toml` - `env:` + commented `env:UNDEFINED`/`env: SPACED`/etc.
- `fuzz/corpus/.../seed-url-sql-injection-shape.toml` - `pass'; DROP TABLE users--` embedded in URL
- `crates/pmcp-server-toolkit/tests/reference_configs.rs` - added seed-parse smoke test
- `CLAUDE.md` - publish-order updated (3 new connector crates)
- `.planning/REQUIREMENTS.md` - CONN/TEST closure + Athena description corrections + footer
- `.planning/phases/84-.../84-VALIDATION.md` - nyquist flip + filled matrix + approval

## Decisions Made

- **Corpus tracking:** committed only the 7 named seeds + the test; the corpus dir holds 5,579 untracked libfuzzer hash-named entries (pre-existing from Phase 77 + this run's additions) that are intentionally not git-tracked, matching the Phase 77/79 convention where only named `seed-*.toml` files are committed. No existing corpus entry was deleted or disturbed.
- **Athena ledger correction:** CONN-07/TEST-01 said "Glue catalog"; the shipped Athena connector uses `GetTableMetadata` with NO `aws-sdk-glue` (Landmine #4 / D-08). Corrected the ledger text to match reality rather than leaving a false claim.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking / stale guard] `aws-sdk-glue` sweep guard interpreted by intent, not literally**
- **Found during:** Task 2 (verification sweep)
- **Issue:** The plan's guard `! grep -rn "aws_sdk_glue\|aws-sdk-glue" crates/pmcp-toolkit-athena/` fails because two intentional documentation comments ("NO aws-sdk-glue …") were deliberately kept per the Wave 0 decision (STATE.md). A literal pass would require deleting the anti-regression doc comments — contradicting the Wave 0 decision.
- **Fix:** Verified the guard's true intent (no Glue in the build graph): `cargo tree -p pmcp-toolkit-athena` shows zero Glue, no `Cargo.toml` dependency, no `use aws_sdk_glue` in source. The only matches are the intentional "NO aws-sdk-glue" doc comments, which were preserved.
- **Files modified:** none (intent satisfied; comments preserved deliberately)
- **Verification:** `cargo tree` + Cargo.toml grep + source-import grep all confirm no Glue.
- **Committed in:** n/a (no code change required)

**2. [Rule 1 - Accuracy] Corrected stale Athena requirement descriptions in REQUIREMENTS.md**
- **Found during:** Task 2 (REQUIREMENTS closure)
- **Issue:** CONN-07 ("Glue catalog-driven schema_text()") and TEST-01 ("Athena `?`+Glue catalog") described an approach the implementation deliberately rejected (Landmine #4 / D-08 — no aws-sdk-glue; uses GetTableMetadata).
- **Fix:** Updated both descriptions to reflect the as-shipped GetTableMetadata approach.
- **Files modified:** `.planning/REQUIREMENTS.md`
- **Verification:** grep confirms the corrected text; intent (Athena schema introspection) unchanged.
- **Committed in:** `0f4d8db2`

---

**Total deviations:** 2 (1 stale-guard reinterpretation, 1 ledger-accuracy correction)
**Impact on plan:** Both keep the closeout truthful; no scope creep. No source code changed.

## Issues Encountered

- **Broad `make quality-gate` blocked by pre-existing unrelated lints.** The full workspace gate (pedantic+nursery `-D warnings` via `--features full`) fails to compile `pmcp-widget-utils` because local rust-1.95.0 fires 4 pedantic lints (`uninlined_format_args`, `must_use`) that CI's pinned-stable toolchain does not. This is documented in `deferred-items.md` and the executor context as an explicit out-of-scope caveat. **Resolution:** scoped the verification sweep to Phase 84's 4 crates instead. All four are confirmed pedantic-clean for their own code via `cargo clippy --no-deps -- -W clippy::pedantic -D warnings`. The widget-utils lints were NOT fixed (per instructions). The broad gate is deferred to CI, where the pinned stable toolchain does not trip these lints.

## TDD Gate Compliance

Plan type is `execute` (not `tdd`). Task 1 added a `test(...)`-typed commit for the corpus + smoke test; no RED/GREEN feature gate sequence applies.

## Threat Flags

None — no new security surface beyond the threat_model already enumerated in the plan (the adversarial seeds exercise the existing `ServerConfig::from_toml` boundary; no real credentials in any seed per T-84-08-03).

## Phase 84 Closure Status

With this plan, **Phase 84 is complete (9/9 plans)**:
- Wave 0: scaffold (84-00)
- Wave 1: SqlConnector.execute + translate_placeholders + build_code_mode_prompt + SQLite (84-01..04)
- Wave 2: Postgres / MySQL / Athena connector crates (84-05..07)
- Wave 3: closeout — fuzz corpus + publish-order + requirements + validation (84-08)

All CONN-01..08, TEST-01, TEST-07 closed. nyquist-compliant.

## Next Phase Readiness

- **Phase 85 (Shape A + REF parity)** is unblocked: the connector trait, all four backends, and the config-parser fuzz coverage are in place.
- **Operator follow-up:** before publishing, run the full `make quality-gate` on the CI-pinned stable toolchain (the local rust-1.95.0 pedantic lints in `pmcp-widget-utils` must either age out as CI advances or be fixed in a dedicated cleanup — tracked in `deferred-items.md`, out of Phase 84 scope).

## Self-Check: PASSED

All 12 created/modified files verified present on disk; both task commits (`84dcddac`, `0f4d8db2`) verified in git history.

---
*Phase: 84-sql-connectors-postgres-mysql-athena-sqlite*
*Completed: 2026-05-26*
