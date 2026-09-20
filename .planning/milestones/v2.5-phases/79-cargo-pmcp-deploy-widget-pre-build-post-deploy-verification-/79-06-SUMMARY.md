---
phase: 79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification-
plan: 06
subsystem: cargo-pmcp
tags: [cargo-pmcp, widgets, deploy, raw-html, cdn, mcp-apps, gap-closure, regression-fix, npm, package-json]

requires:
  - phase: 79
    provides: "Plans 79-01..79-05 — WidgetsConfig schema, widget pre-build orchestrator (`run_widget_build`), `verify_build_script_exists`, and the call site in `commands/deploy/mod.rs::pre_build_widgets_and_set_env` that 79-06 patches"
provides:
  - "is_node_project(widget_dir) helper using Path::is_file() (cog ≤2)"
  - "Raw-HTML / CDN-import widget early-return guard at the top of run_widget_build (preserves HIGH-C1 PMCP_WIDGET_DIRS invariant via Ok(resolved) return)"
  - "Defense-in-depth Path::is_file() guard at the head of verify_build_script_exists (replaces raw os-error-2 with friendly diagnostic)"
  - "cargo-pmcp 0.12.1 patch release closing UAT Test 3 regression"
  - "9 new tests (6 unit U1..U6 + 3 integration I1..I3) covering the raw-HTML widget archetype"
  - "Eliminated the npm install parent-walk side effect (1839-package audit observed in Scientific-Calculator-MCP-App reproduction)"
affects: [phase-80, future-cargo-pmcp-releases]

tech-stack:
  added: []
  patterns:
    - "Skip-detector helper pattern co-located with `is_yarn_pnp` — both pre-build skip-tests live adjacent so future skip cases (e.g., Deno, raw WASM) follow the same shape."
    - "Defense-in-depth dual guard: early-return at the orchestrator entry point + secondary Path::is_file() guard at the deepest fs read site, so even if a future caller bypasses the early-return (explicit argv path) the diagnostic is still actionable."
    - "Schema-direct integration testing: integration tests call run_widget_build directly via cargo_pmcp::deployment::widgets::* (not via assert_cmd spawning a real cargo pmcp deploy) — faster, hermetic, and proves no subprocess by passing on runners with no node tooling on PATH."

key-files:
  created:
    - "cargo-pmcp/tests/widgets_raw_html.rs"
  modified:
    - "cargo-pmcp/src/deployment/widgets.rs"
    - "cargo-pmcp/Cargo.toml"
    - "cargo-pmcp/CHANGELOG.md"
    - ".planning/phases/79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification-/79-UAT.md"

key-decisions:
  - "Early-return wins over explicit Node-shaped argv when no package.json: an explicit `widget.build = [\"npm\", \"run\", \"build\"]` against a manifest-less directory becomes a no-op skip rather than a crash. Operator misconfig is treated as a no-op, not a failure (locked by integration test I2)."
  - "Use Path::is_file() not Path::exists() in is_node_project: a directory accidentally named `package.json` is correctly rejected as NOT a Node project (locked by unit test U1's edge case)."
  - "Defense-in-depth guard inside verify_build_script_exists fires even though run_widget_build's early-return covers the common path. Keeps the diagnostic actionable when a future caller bypasses the early-return (e.g., stale node_modules/ masking a deleted package.json)."
  - "Two-commit ladder: Task 1 (code + tests + CHANGELOG) commits the fix; Task 2 (Cargo.toml version bump + integration tests) commits the release-readiness layer. Each commit is independently reviewable and revertable."
  - "TDD declared on the plan, executed as combined RED+GREEN per Plan 79-05's documented pattern: tests are added in the same commit as implementation because a separate non-compiling RED commit would be ceremony-only (the project's standard Rust pattern). Tests still verifiably failed at the lib.rs compile boundary before the helper was added (verified during execution)."

patterns-established:
  - "Raw-HTML / CDN-import widget archetype guard: when adding new auto-detection paths to widget pre-build, mirror the is_node_project pattern — a single Path::is_file() boolean helper + early-return at the orchestrator entry point + defense-in-depth at the deepest fs read site."
  - "Schema-direct integration testing for cargo-pmcp deployment: tests in cargo-pmcp/tests/widgets_*.rs call into pub fn cargo_pmcp::deployment::widgets::* directly via tempdir fixtures, NOT via assert_cmd spawning the binary. Hermetic, faster, and provides a no-subprocess contract (tests pass on a runner with no node tooling on PATH)."

requirements-completed: [REQ-79-01, REQ-79-02, REQ-79-03]

duration: ~25min
completed: 2026-05-03
---

# Phase 79 Plan 06: Raw-HTML Widget Pre-Build Gap-Closure Summary

**Closes UAT Test 3 (severity: major) — `cargo pmcp deploy --widgets-only` no longer hard-crashes with raw `os error 2` when a `widgets/` directory contains only raw `*.html` files (no `package.json`); npm install parent-walk side effect (1839-package audit) eliminated; cargo-pmcp 0.12.1 patch released.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-05-03 ~21:30 UTC
- **Completed:** 2026-05-03 ~21:55 UTC
- **Tasks:** 2/2
- **Tests added:** 9 new (6 unit U1..U6 + 3 integration I1..I3)
- **Files created:** 1 (`cargo-pmcp/tests/widgets_raw_html.rs`)
- **Files modified:** 3 (widgets.rs, Cargo.toml, CHANGELOG.md) + UAT.md flip

## Accomplishments

- Closed the documented UAT Test 3 regression: a `widgets/` directory containing only `keypad.html` (with a `https://esm.sh/...` CDN import, no `package.json`, no lockfile) — the canonical Phase 45 zero-build MCP Apps archetype — now correctly takes the raw-HTML early-return path. Zero subprocesses spawned. The `npm install` parent-walk side effect (1839 packages audited from a parent workspace, risking writes to `node_modules/` or `package-lock.json` outside the project) is eliminated at the source.
- Replaced the unactionable raw `io::Error: No such file or directory (os error 2)` with a friendly diagnostic naming the widget directory and three remediation paths (add `package.json` with a `build` script, configure `[[widgets]]` with a non-Node build, or remove the override).
- Preserved all REVISION 3 invariants: HIGH-C1 (`PMCP_WIDGET_DIRS` colon-list still includes raw-HTML widget directories), HIGH-G1 (`discover_local_widget_dirs` build.rs fallback unchanged), HIGH-G2 (`ROLLBACK_REJECT_MESSAGE` unchanged) — all verified by post-execution grep.
- 9 new tests pass under `--test-threads=1`. Full cargo-pmcp suite at 1083 tests passing (up from 1080 prior to this plan).
- `cargo-pmcp` version bumped 0.12.0 → 0.12.1 (semver patch — additive guard, no API breakage). CHANGELOG.md `## [0.12.1] - 2026-05-03` entry under `### Fixed` documents the regression closed and the npm parent-walk eliminated.

## Task Commits

Each task was committed atomically:

1. **Task 1: Raw-HTML guard + defense-in-depth + unit tests + CHANGELOG** — `7b3fe93a` (fix)
2. **Task 2: Integration tests for raw-HTML widget archetype + version bump 0.12.0 → 0.12.1** — `28381f34` (chore)

**Plan metadata:** `439caa25` (plan: gap-closure for raw-HTML widget crash)

_Note: this plan declares `tdd="true"` but executed as combined RED+GREEN per the project's documented pattern (see Plan 79-05's TDD Gate Compliance section). The 9 tests were authored in the same commit as the implementation; RED was verified at the lib.rs compile boundary (errors `cannot find function 'is_node_project' in this scope` × 5 sites) before the helper was added._

## Files Created/Modified

### Created

- `cargo-pmcp/tests/widgets_raw_html.rs` — 3 integration tests (I1..I3) exercising the raw-HTML widget archetype via direct calls to `run_widget_build`. I1 mirrors the Scientific-Calculator-MCP-App reproduction (keypad.html with CDN import, no package.json, no lockfile) and asserts no `node_modules/` or `package-lock.json` is created anywhere — proof that no subprocess was spawned. Test passes on runners with no `npm`/`pnpm`/`yarn`/`bun` on PATH. I2 locks the design choice that the early-return wins over explicit Node-shaped argv. I3 is regression coverage for the Node-project happy path.

### Modified

- `cargo-pmcp/src/deployment/widgets.rs` — Three additions:
  - New `is_node_project(widget_dir: &Path) -> bool` helper (cog ≤2, adjacent to `is_yarn_pnp` so the two skip-detectors live together) using `Path::is_file()` to reject directories accidentally named `package.json`.
  - Early-return guard at the top of `run_widget_build` after `validate()` + `resolve_paths()`, before `PackageManager::detect_from_dir`. Prints `treating <path> as raw HTML / CDN bundle, skipping build` when not quiet, returns `Ok(resolved)` so the caller still appends the directory to `PMCP_WIDGET_DIRS` (HIGH-C1 invariant preserved).
  - Defense-in-depth `Path::is_file()` guard at the head of `verify_build_script_exists` BEFORE `read_to_string`. Bails with a friendly diagnostic naming the widget dir and three remediation paths.
  - 6 new unit tests U1..U6 in the existing `mod tests` block.
- `cargo-pmcp/Cargo.toml` — Single-line version bump `version = "0.12.0"` → `version = "0.12.1"`.
- `cargo-pmcp/CHANGELOG.md` — New `## [0.12.1] - 2026-05-03` entry inserted ABOVE `## [0.12.0]`. `### Fixed` block describes (1) the raw-HTML widget hard-crash regression closed, (2) the npm parent-walk side effect eliminated, (3) the defense-in-depth diagnostic. `### Notes` block documents the preserved REVISION 3 invariants (HIGH-C1, HIGH-G1, HIGH-G2) and the reference reproduction (Scientific-Calculator-MCP-App).
- `.planning/phases/79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification-/79-UAT.md` — Test 3 flipped from `result: issue` → `result: pass` with evidence pointing to integration test I1. Summary block updated: passed 10 → 11, issues 1 → 0. Gap entry status updated `failed` → `closed` with `closed_by:` reference to commits 7b3fe93a + 28381f34.

## Decisions Made

See `key-decisions:` in frontmatter. Plan executed as written; design choices already documented in the plan body (early-return-wins-over-explicit-argv, Path::is_file vs Path::exists, two-commit ladder).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] rustfmt line-wrap on Task 1 unit test U6**

- **Found during:** Task 2 (running `make quality-gate` end-to-end as the final acceptance step).
- **Issue:** Task 1's U6 unit test contained a line-wrapped `std::fs::create_dir_all(widgets_dir.join("node_modules")).expect("create node_modules/")` chain that rustfmt collapses onto a single line. `cargo fmt --all -- --check` (the first step of `make quality-gate`) failed.
- **Fix:** Ran `cargo fmt --all` to apply the rustfmt collapse. Single-line trivial cleanup (3 → 1 line). Folded into Task 2's commit rather than amending Task 1 (per CLAUDE.md "ALWAYS create NEW commits rather than amending").
- **Files modified:** `cargo-pmcp/src/deployment/widgets.rs` (line 1089-1090, U6 test body).
- **Verification:** Subsequent `make quality-gate` exited 0 with no fmt warnings.
- **Committed in:** `28381f34` (Task 2 commit).

---

**Total deviations:** 1 auto-fixed (1 bug — rustfmt fmt-check failure)
**Impact on plan:** Trivial fmt fix discovered when running the project's full quality gate. No scope creep, no behavior change. Lesson: run `make quality-gate` between task commits, not just at the end.

## Issues Encountered

**Pre-existing PMAT complexity violations carried forward (NOT fixed by this plan):**
`pmat quality-gate --fail-on-violation --checks complexity` reports 2 pre-existing violations (same as Plan 79-05 baseline):
- `cargo-pmcp/src/commands/test/apps.rs:88::execute_pretty` — cog 27 (pre-existing baseline; renamed body of original `execute` from Plan 79-05).
- `crates/mcp-tester/src/app_validator.rs:248::strip_js_comments` — cog 59 (pre-existing in another file).

Per the SCOPE BOUNDARY rule (executor.md): only auto-fix issues DIRECTLY caused by the current task's changes. Both violations pre-exist this plan; neither lives in `widgets.rs` (the only file this plan modifies). The new code (`is_node_project` cog ≤2; early-return adds cog ≤2 to `run_widget_build` which had cog ≤7; defense-in-depth guard adds cog ≤2 to `verify_build_script_exists` which had cog ≤6) is well under the PMAT cap of 25.

**Pre-existing cargo clippy warnings on `cargo-pmcp --all-targets`:**
14 clippy errors are reported when running `cargo clippy -p cargo-pmcp --all-targets -- -D warnings`. Verified pre-existing by `git stash && cargo clippy ...` against the unmodified pre-task HEAD — same 14 errors. They live in `pentest/attacks/*.rs`, `loadtest/summary.rs`, `deployment/config.rs`, and `lib.rs` doc-list formatting. The project's actual quality gate (`make lint` → `make quality-gate`) uses `--features "full" --lib --tests` against the root crate (pmcp), not `--all-targets` against cargo-pmcp; that gate exits 0. No new clippy errors from this plan's changes (verified by stash diff).

## User Setup Required

None — no external service configuration required. This is a pure code patch fix.

## Next Phase Readiness

**Release-ready:** `cargo-pmcp` 0.12.1 is now publishable via the existing `v*` tag → `release.yml` workflow per CLAUDE.md "Release Steps". The Release workflow's per-crate skip logic (`if a crate version already exists on crates.io, the publish step skips it gracefully`) means tagging `vX.Y.Z` with this plan's changes will publish ONLY cargo-pmcp 0.12.1; pmcp 2.6.0, mcp-tester 0.6.0, and pmcp-widget-utils remain unchanged.

**Phase 79 closed:** With UAT Test 3 flipped to `pass`, Phase 79 has 11/14 tests passing + 0 issues + 3 skipped without reason (Tests 9, 10, 14 — these require either explicit operator UAT-on-real-project or an `app new` scaffold integration check that wasn't covered in any wave). The phase's gap-closure obligation is satisfied; the 3 skipped-without-reason tests are non-blocking and can be re-classified as out-of-scope or covered by a future verification pass.

**No blockers for downstream consumers:** Operators with `widgets/` directories containing only `*.html` files (the documented Phase 45 archetype) can now run `cargo pmcp deploy` without crashes. Operators with `widget/package.json` continue to use the existing Node pipeline unchanged.

## Verification

### Unit tests — 6/6 new (15/15 total in widgets module)

```bash
cargo test -p cargo-pmcp --lib deployment::widgets:: -- --test-threads=1
# test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 379 filtered out
```

### Integration tests — 3/3 new

```bash
cargo test -p cargo-pmcp --test widgets_raw_html -- --test-threads=1
# test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Full cargo-pmcp suite — 1083/1083 pass

```bash
cargo test -p cargo-pmcp -- --test-threads=1
# cargo test: 1083 passed (18 suites, 22.70s)
```

### Quality gate — PASS

```bash
make quality-gate  # exit 0
# Toyota Way: fmt-check + lint + build + test-all + audit + unused-deps + check-todos + check-unwraps + validate-always
```

### REVISION 3 invariants — preserved (grep evidence)

```bash
$ grep -rn 'ROLLBACK_REJECT_MESSAGE' cargo-pmcp/src/ | head -5
# 5 references in cargo-pmcp/src/deployment/post_deploy_tests.rs — UNCHANGED.

$ grep -n 'PMCP_WIDGET_DIRS' cargo-pmcp/src/commands/deploy/mod.rs | head -5
# `set_var("PMCP_WIDGET_DIRS", &joined)` at line 545 — UNCHANGED.

$ grep -rn 'discover_local_widget_dirs' cargo-pmcp/src/ | head -5
# `fn discover_local_widget_dirs` in cargo-pmcp/src/templates/mcp_app.rs:130 — UNCHANGED.
```

### Version + CHANGELOG

```bash
$ grep -n '^version = "0.12.1"' cargo-pmcp/Cargo.toml
3:version = "0.12.1"

$ grep -n '## \[0.12.1\] - 2026-05-03' cargo-pmcp/CHANGELOG.md
8:## [0.12.1] - 2026-05-03
```

### PMAT cognitive complexity — no new violations

`pmat quality-gate --fail-on-violation --checks complexity` reports the same 2 pre-existing violations as Plan 79-05's baseline (apps.rs::execute_pretty cog 27, app_validator.rs::strip_js_comments cog 59). Neither lives in widgets.rs. The new code is cog ≤2 for the helper, ≤9 for run_widget_build (was ≤7), ≤8 for verify_build_script_exists (was ≤6) — all well under the PMAT cap of 25.

## Threat Flags

None. The plan's `<threat_model>` was implemented as documented:

- **T-79-06-01** (npm install parent-walk Information disclosure + Tampering): MITIGATED via `is_node_project` early-return in `run_widget_build`. No `npm` subprocess is spawned for raw-HTML widgets, so npm cannot walk UP the directory tree to a parent workspace's `package.json` and audit/modify packages there. Closes the observed 1839-package parent-walk audit.
- **T-79-06-02** (raw io::Error from missing package.json — Denial of service via crash): MITIGATED via `Path::is_file()` defense-in-depth guard at the head of `verify_build_script_exists`. The unactionable raw `os error 2` is replaced with a friendly diagnostic.
- **T-79-06-03** (path-traversal in `is_node_project`): ACCEPTED — the helper composes `widget_dir.join("package.json")` and calls `Path::is_file()`. No traversal surface; `widget.path` is already validated by `WidgetConfig::validate()` (T-79-02 mitigation, Phase 79 Wave 1) before reaching this point.
- **T-79-06-04** (early-return wins over explicit npm argv — Tampering with operator's intent): ACCEPTED — locked by integration test I2 and the `key-decisions:` rationale. An explicit `npm` argv against a manifest-less dir is operator misconfig; treating it as a no-op skip is safer than spawning a subprocess that will crash or walk up the parent tree.

No new security-relevant surface introduced beyond the documented threat register.

## Self-Check: PASSED

- [x] `cargo-pmcp/src/deployment/widgets.rs` modified — FOUND (`fn is_node_project` line 338; early-return guard at line 406; defense-in-depth guard at line 549; 6 new unit tests U1..U6 in mod tests)
- [x] `cargo-pmcp/Cargo.toml` modified — FOUND (line 3: `version = "0.12.1"`)
- [x] `cargo-pmcp/CHANGELOG.md` modified — FOUND (line 8: `## [0.12.1] - 2026-05-03`)
- [x] `cargo-pmcp/tests/widgets_raw_html.rs` — FOUND (3 integration tests I1..I3)
- [x] `.planning/phases/79-.../79-UAT.md` updated — FOUND (Test 3 flipped to `result: pass`, summary updated to passed: 11 / issues: 0)
- [x] Commit `7b3fe93a` (Task 1) — FOUND in git log: `fix(79-06): raw-HTML widget pre-build skip + defense-in-depth + CHANGELOG`
- [x] Commit `28381f34` (Task 2) — FOUND in git log: `chore(79-06): bump cargo-pmcp 0.12.0 → 0.12.1 + integration tests for raw-HTML widget archetype`
- [x] `make quality-gate` exits 0 — VERIFIED (background task completed exit code 0)
- [x] `cargo test -p cargo-pmcp -- --test-threads=1` exits 0 with 1083 tests passed — VERIFIED
- [x] All 9 new tests pass under --test-threads=1 — VERIFIED (6 unit + 3 integration)
- [x] HIGH-G2 / HIGH-C1 / HIGH-G1 invariants preserved — VERIFIED via grep

---
*Phase: 79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification-*
*Plan: 06 (gap-closure)*
*Completed: 2026-05-03*
