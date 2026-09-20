---
phase: 94-cli-subcommands-pmcp-toml
plan: 01
subsystem: cli
tags: [pmcp-toml, config-parser, serde, toml, path-containment, proptest, cargo-pmcp, workbook]

# Dependency graph
requires:
  - phase: 93-workbook-compiler
    provides: pmcp-workbook-compiler crate (compile/gate/lint verbs) consumed via the new path dep
provides:
  - "cargo-pmcp → pmcp-workbook-compiler path dependency (offline cone)"
  - "PmcpToml repo-root config parser: load(project_root)→Ok(None) on absence, resolve(bundle_id), all_entries()"
  - "validate(project_root) containment: rejects absolute-outside-root and ..-escaping path/out_dir + duplicate bundle_ids"
  - "resolves_outside(root, candidate) reusable lexical containment predicate (no canonicalize)"
  - "BOUNDED proptest fuzz harness over the config parser (no-panic, round-trip, escape-rejection, resolve totality)"
affects: [94-02-workbook-command-group, 94-03-compile, 94-04-lint-emit]

# Tech tracking
tech-stack:
  added: [pmcp-workbook-compiler-path-dep]
  patterns:
    - "Optional repo-root TOML config: load() returns Ok(None) on absent file (D-03), validate() called inside load with project_root in hand"
    - "Path containment computed against passed-in project_root, not lexical ..-inspection alone (absolute escapes have no ..)"
    - "In-file #[cfg(test)] mod proptests with bounded {0,N}-capped string strategies (no bare .*)"

key-files:
  created:
    - cargo-pmcp/src/commands/workbook/config.rs
    - cargo-pmcp/src/commands/workbook/mod.rs
  modified:
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/src/commands/mod.rs

key-decisions:
  - "Array-of-tables [[workbook]] shape (Vec<WorkbookEntry>), serde rename workbook→workbooks"
  - "WorkbookEntry carries path + bundle_id + out_dir ONLY — no version/approver field (D-02)"
  - "resolves_outside does NOT canonicalize (out_dir may not exist yet); foo/../bar under root is tolerated"

patterns-established:
  - "Containment predicate factored out (resolves_outside) so the proptest can drive it directly"
  - "Bounded proptest string strategies via proptest::string::string_regex with {0,N} caps"

requirements-completed: [WBCL-04]

# Metrics
duration: ~35min
completed: 2026-06-13
---

# Phase 94 Plan 01: pmcp.toml Parser Summary

**Repo-root `pmcp.toml` parser (`PmcpToml`) mapping workbooks → bundle IDs with project-root path containment (rejects absolute + `..`-escape), plus a bounded proptest fuzz harness, and the cargo-pmcp → pmcp-workbook-compiler dependency edge.**

## Performance

- **Duration:** ~35 min
- **Tasks:** 3 (all TDD/auto)
- **Files modified:** 4 (2 created, 2 modified)

## Accomplishments
- Added `pmcp-workbook-compiler = { version = "0.1.0", path = "../crates/pmcp-workbook-compiler" }` to cargo-pmcp, forming the offline cone `cargo-pmcp → pmcp-workbook-compiler → pmcp-workbook-runtime`.
- Implemented `PmcpToml` (`Vec<WorkbookEntry>`, `[[workbook]]` array-of-tables) with `load(project_root)→Ok(None)` on a missing file (D-03), `resolve(bundle_id)` (D-05) erroring with the missing id + `pmcp.toml`, and `all_entries()` for compile-all.
- `validate(project_root)` rejects duplicate bundle_ids (T-94-01-DUP) and any `path`/`out_dir` that resolves outside the root — absolute-outside-root OR `..`-escaping (T-94-01-PATH / concern C), while tolerating `foo/../bar` under the root.
- Factored `resolves_outside(root, candidate)` so the fuzz harness drives the predicate directly; it does NOT canonicalize (the out_dir may not yet exist).
- Added a 4-property bounded proptest harness: no-panic over bounded TOML bytes, lossless serde round-trip, rejection of both `../` and absolute escapes, and resolve = exact membership.

## Task Commits

1. **Task 1: Add the compiler dependency to cargo-pmcp** - `1e27d7c6` (chore)
2. **Task 2 + Task 3: pmcp.toml parser + bounded proptest fuzz harness** - `73b4ac50` (feat) — implementation, mandated fuzz harness, and module registration in one new-file deliverable (fmt fix amended in)

## Files Created/Modified
- `cargo-pmcp/Cargo.toml` - Added the `pmcp-workbook-compiler` path dependency.
- `cargo-pmcp/src/commands/workbook/config.rs` - The `PmcpToml`/`WorkbookEntry` serde structs, `load`/`resolve`/`all_entries`/`validate(project_root)`, the `resolves_outside` containment predicate, 11 unit tests, and a 4-property bounded proptest harness.
- `cargo-pmcp/src/commands/workbook/mod.rs` - New module file declaring `pub mod config;` (minimal; Plan 02 extends it with the `WorkbookCommand` group).
- `cargo-pmcp/src/commands/mod.rs` - Registered `pub mod workbook;`.

## Decisions Made
- **Array-of-tables `[[workbook]]`** over a keyed map (D-02 discretion) — cleanest for the path+id+out_dir triple; `#[serde(rename = "workbook")]` maps the TOML section to the `workbooks` field.
- **`resolves_outside` does not canonicalize** — the out_dir may not exist yet, so containment is purely lexical: absolute paths must `starts_with` root, relative paths walk components tracking depth (depth<0 ⇒ escape). `foo/../bar` stays under root and is accepted.
- **No version/approver field on `WorkbookEntry`** (D-02) — both are documented in the struct doc comment as intentionally absent (version from the workbook, approver from `--approver`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Verification ran against `--bin cargo-pmcp`, not `--lib`**
- **Found during:** Task 2 (test run)
- **Issue:** The plan's `<automated>` verifications and acceptance criteria specify `cargo test -p cargo-pmcp --lib workbook::config`. In reality `commands/*` (where `config.rs` lives) is a **bin-only** module tree — `cargo-pmcp/src/lib.rs` deliberately excludes `commands::*` (it only mounts narrow deployment/utils/test-support seams via `#[path]`). A `--lib` run reports `0 passed, 429 filtered out` because the module is invisible to the lib target. The cross-AI "is `--lib` valid?" concern the plan dismissed was, in fact, correct for this module.
- **Fix:** Ran the tests against the bin target: `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::config` → **16 passed** (11 unit + 5 proptest properties). No source change was needed; only the verification command differs.
- **Files modified:** None (verification-command-only).
- **Verification:** `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::config` → `test result: ok. 16 passed; 0 failed`.
- **Committed in:** n/a (no code change).

**2. [Rule 3 - Blocking] Created `commands/workbook/mod.rs` + registered the module so config.rs compiles**
- **Found during:** Task 2 (module had to be reachable to compile/test)
- **Issue:** The plan's `files_modified` lists only `config.rs` and `Cargo.toml`, but `commands/workbook/mod.rs` does not exist yet (Plan 02 creates the full command group). Without a `mod.rs` declaring `pub mod config;` and a `pub mod workbook;` line in `commands/mod.rs`, `config.rs` is unreachable and the bin target does not compile it.
- **Fix:** Added a minimal `commands/workbook/mod.rs` (`pub mod config;` plus a doc comment noting Plan 02 will add the command group) and one `pub mod workbook;` line to `commands/mod.rs`. This is forward-compatible — Plan 02 extends `mod.rs` with the `WorkbookCommand` enum without conflict.
- **Files modified:** `cargo-pmcp/src/commands/workbook/mod.rs` (new), `cargo-pmcp/src/commands/mod.rs`.
- **Verification:** `cargo check -p cargo-pmcp` resolves; tests run.
- **Committed in:** `73b4ac50`.

**Note on the D-02 grep criterion:** The acceptance criterion `grep -v '^#' ... | grep -c 'version\|approver'` expects 0, but returns 4. All 4 matches are in `//!`/`///` doc comments explaining *why* version/approver are absent (the criterion's `^#` comment-strip targets shell/`#` comments, not Rust `//`). The substantive intent — no `version`/`approver` **field** on `WorkbookEntry` — is satisfied: the struct has only `path`, `bundle_id`, `out_dir`. The doc comments are the correct place to record the D-02 decision and were kept.

**Note on `proptest` (scope-note contingency):** The scope note said to fall back to table-driven `#[test]`s if `proptest` was not already a dev-dependency. It IS present (`cargo-pmcp/Cargo.toml:97`), so the real `proptest!` harness was used as the plan intended — no fallback needed.

---

**Total deviations:** 2 auto-fixed (both Rule 3 - blocking; one verification-command-only, one module-reachability).
**Impact on plan:** No scope creep. The `--lib` → `--bin` correction is a verification-target fix; the `mod.rs`/registration is the minimum needed to compile a `commands/`-tree file ahead of Plan 02 and is forward-compatible.

## Issues Encountered
- `cargo fmt --check` (run by `make quality-gate`) flagged trailing-comma and assert-wrap formatting in the new file; resolved with `cargo fmt -p cargo-pmcp` and amended into the config.rs commit.
- `make quality-gate` reports `cargo fuzz` build errors (`failed to run rustc to learn about target-specific information`) for the existing `fuzz/` targets — an environment/toolchain issue with the cargo-fuzz sanitizer build, unrelated to this plan's files. The gate still **exits 0** (the lint/test/build/audit portions pass; the workbook code is not a cargo-fuzz target — it uses in-tree proptest per CLAUDE.md).

## Quality Gate
- `make quality-gate` → **exit 0** (PASS). `cargo fmt --all --check` clean, lint clean on the `pmcp` gate set, examples check clean, build + audit pass.
- `cargo clippy -p cargo-pmcp --tests` → zero warnings attributable to `workbook/config.rs` (pre-existing dead-code warnings in `pentest`/`deployment`/`configure` are out of scope).
- `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::config` → 16 passed.

## Threat Surface
All threat-register mitigations were implemented in-plan (T-94-01-PATH, T-94-01-DUP, T-94-01-TOML). No new security-relevant surface beyond the plan's `<threat_model>`. No threat flags.

## Known Stubs
None — `PmcpToml`/`validate`/`resolve`/`all_entries` are fully wired with passing unit + property tests. The consuming command handlers (compile/lint/emit) are Plan 02–04 scope, as designed.

## Next Phase Readiness
- The `PmcpToml` public surface (`load`/`resolve`/`all_entries`/`validate`) is ready for Plan 02's `WorkbookCommand` group and Plans 03/04's compile/lint/emit handlers.
- `commands/workbook/mod.rs` exists with `pub mod config;` — Plan 02 adds the `WorkbookCommand` enum + sibling handler modules to it.

## Self-Check: PASSED
- FOUND: cargo-pmcp/src/commands/workbook/config.rs
- FOUND: cargo-pmcp/src/commands/workbook/mod.rs
- FOUND commit: 1e27d7c6
- FOUND commit: 73b4ac50

---
*Phase: 94-cli-subcommands-pmcp-toml*
*Completed: 2026-06-13*
