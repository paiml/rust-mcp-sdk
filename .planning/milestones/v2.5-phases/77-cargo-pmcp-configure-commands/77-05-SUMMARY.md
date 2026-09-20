---
phase: 77
plan: 05
subsystem: cargo-pmcp/cli
tags: [configure, list, show, json-output, btreemap-order, fixed-field-order, banner-d13]
dependency_graph:
  requires:
    - "77-03 (TargetConfigV1 reader, TargetEntry::type_tag, default_user_config_path)"
    - "77-04 (read_active_marker — BOM/whitespace-tolerant marker reader)"
  provides:
    - "configure::list::execute — text + stable JSON output, BTreeMap-ordered, env/marker active-source detection"
    - "configure::list::compute_active_target — simplified resolver (env > marker > none) used by list"
    - "configure::show::execute — explicit-name or fall-through-to-active inspection, --raw + merged forms"
    - "configure::show::collect_for_display — fixed banner-order field tuple (api_url, aws_profile, region, extras) ready for Plan 06 to enrich with real source attribution"
  affects:
    - cargo-pmcp/src/commands/configure/list.rs (stub → 370 lines: ListArgs + execute + 3 helpers + 6 tests)
    - cargo-pmcp/src/commands/configure/show.rs (stub → 338 lines: ShowArgs + execute + 4 helpers + 6 tests)
tech_stack:
  added: []
  patterns:
    - "Stable JSON shape with snake_case `active_source` enum (Env / WorkspaceMarker / None) — REQ-77-01 contract for scriptable list consumption"
    - "Owned TargetEntry::clone wrapper for `--raw` TOML serialization — sidesteps serde's borrowed-ref handling on internally-tagged enums (`#[serde(tag = \"type\")]` on a `&TargetEntry` reference)"
    - "Fixed-banner-field display order (D-13) wired through `collect_for_display` returning `(api_url, aws_profile, region, extras)` — Plan 06's resolver will replace `print_merged_with_attribution`'s body but the helper's contract stays stable"
    - "Per-test HOME+CWD+PMCP_TARGET isolation via `#[serial_test::serial]` + saved-env restore (mirrors Plan 04's helper pattern, plus PMCP_TARGET save/restore)"
key_files:
  created:
    - .planning/phases/77-cargo-pmcp-configure-commands/77-05-SUMMARY.md
  modified:
    - cargo-pmcp/src/commands/configure/list.rs
    - cargo-pmcp/src/commands/configure/show.rs
key_decisions:
  - "Single commit per task (test + impl co-located) — matches Plan 03/04 precedent. Tests live in `#[cfg(test)] mod tests` inside the same file as the implementation."
  - "Owned-clone TargetEntry inside the `--raw` wrapper map. The plan body anticipated the issue: 'if serde-with-borrow trips up, change to .cloned()'. Cloning is cheap (the largest variant is 4 strings) and avoids the lifetime gymnastics of serializing a `BTreeMap<String, &TargetEntry>` through a tagged enum."
  - "ActiveSource derives `Copy` (cheap 3-variant enum) so `print_text` / `print_json` take it by value rather than reference — avoids one pointer indirection per call site without complicating the API."
  - "compute_active_target NOT extracted into a shared helper module yet — Plan 06's resolver will subsume it. Keeping the simplified version inline in list.rs avoids speculative module structure; show.rs duplicates the env-check + workspace-walk inline in `resolve_active_or_fail` for the same reason. Plan 09 quality-gate cleanup may consolidate."
  - "ShowArgs.name is `Option<String>` (clap positional) so `cargo pmcp configure show` (no arg) falls through to the active marker, matching the plan's behavior. The `--raw` flag uses `#[arg(long)]` (no short alias) so a future `-r` is reserved for region-override."
  - "All 6 show tests are `#[serial]` (env mutation) except the 2 pure `collect_for_display` tests, which are deterministic and parallel-safe."
patterns_established:
  - "JSON output convention for inspection commands: `{ schema_version, <selector>, <selector_source>, items[] }` where `<selector_source>` is a snake_case enum. Plans 06+ should match this when they add resolver output."
  - "Read-side subcommand structure: `pub fn execute(args, &gf) -> Result<()>` reads config → resolves selector → dispatches to `print_*` helpers. Output channels are explicit (data → stdout, status/notes → stderr per D-11)."
requirements_completed: []  # REQ-77-01 partially; full close happens after Plan 06 (resolver) + Plan 07 (CLI wiring)
metrics:
  duration: ~25m
  completed: 2026-04-26
  tasks_completed: 2
  files_modified: 2
  files_created: 0
  tests_added: 12
---

# Phase 77 Plan 05: configure list + configure show Summary

**`configure list` (text + stable JSON `{ schema_version, active, active_source, targets[] }` shape, BTreeMap-ordered, env-overrides-marker visibility) and `configure show` (explicit name OR fall-through-to-active, `--raw` TOML block dump OR fixed-banner-order display with `(source: target)` placeholder ready for Plan 06's resolver enrichment) — both read-side / inspection subcommands of the configure group landed with 12 unit tests passing.**

## Performance

- **Duration:** ~25m
- **Started:** 2026-04-26 (sequential executor on main worktree)
- **Completed:** 2026-04-26
- **Tasks:** 2
- **Files modified:** 2
- **Files created:** 0
- **Tests added:** 12 (6 list + 6 show)

## Accomplishments

- `cargo pmcp configure list` is now a real subcommand: enumerates `~/.pmcp/config.toml` targets in deterministic BTreeMap (alphabetical) order with `*` marker on the active row, supports `--format json` for scripts (stable shape: `schema_version: u32`, `active: Option<String>`, `active_source: snake_case_enum`, `targets[]: { name, type, fields, active }`), surfaces `PMCP_TARGET` env override visibility via `active_source: env` + stderr note in text mode, and gracefully prints a "no targets — run `configure add`" hint when the config file is empty/missing.
- `cargo pmcp configure show [<name>]` is now a real subcommand: with explicit name shows the merged form with per-field source attribution (currently always `(source: target)` — Plan 06 fills in env/flag/deploy.toml resolution); with no name falls through to the active marker (`PMCP_TARGET` env > `<workspace>/.pmcp/active-target` > actionable error mentioning both `configure use` and the explicit-name invocation); `--raw` flag dumps just the stored TOML block. Errors with "target 'foo' not found … run `configure add foo`" on unknown names. D-13 banner field ordering (api_url, aws_profile, region, extras) is hard-wired via `collect_for_display` so Plan 06 can replace `print_merged_with_attribution`'s body without touching the field-order contract.
- 12 unit tests pass with `--test-threads=1`. No new clippy errors in `configure/list.rs` or `configure/show.rs`. Pre-existing pentest/loadtest/deployment clippy errors unaffected (out-of-scope per SCOPE BOUNDARY).

## Task Commits

Each task was committed atomically (test + implementation co-located in the same file — single commit per task, matching Plan 03/04 precedent):

1. **Task 1: configure list — text + stable JSON output with active marker** — `27be341a` (feat)
2. **Task 2: configure show — merged or raw target inspection** — `cb4fd522` (feat)

## Files Modified

### Modified (2)

- `cargo-pmcp/src/commands/configure/list.rs` — stub (20 lines) → full impl (370 lines: `ListArgs`, `ListJsonOutput`, `ActiveSource`, `TargetJson`, `execute`, `compute_active_target`, `print_text`, `print_json`, `field_summary`, 6 tests)
- `cargo-pmcp/src/commands/configure/show.rs` — stub (20 lines) → full impl (338 lines: `ShowArgs`, `execute`, `resolve_active_or_fail`, `print_raw`, `print_merged_with_attribution`, `collect_for_display`, 6 tests)

## Decisions Made

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Single commit per task (test + impl) | Matches Plan 03/04 precedent. Tests in `#[cfg(test)] mod tests` inside same file. | 2 commits total. |
| `--raw` wrapper uses owned `TargetEntry::clone` | Plan body anticipated this: "if serde-with-borrow trips up, change to `.cloned()`". `BTreeMap<String, &TargetEntry>` through `#[serde(tag = "type")]` is awkward to satisfy lifetime-wise, and clone is cheap (largest variant = 4 strings). | `print_raw` serializes via owned `BTreeMap<&str, BTreeMap<String, TargetEntry>>` with no borrow-checker friction. |
| `ActiveSource: Copy` | 3-variant fieldless enum — `Copy` lets us pass it by value into `print_text` / `print_json` instead of reference, removing one indirection per call site for negligible cost. | Cleaner signatures: `print_text(cfg, path, active, source)` not `print_text(cfg, path, active, &source)`. |
| `compute_active_target` inlined in list.rs (NOT extracted to a shared module) | Plan 06's resolver will subsume it; speculative module structure now would just churn on Plan 06's first commit. show.rs duplicates the env-check + workspace-walk inline in `resolve_active_or_fail` for the same reason. | Plan 09 may DRY this during quality-gate cleanup. |
| `ShowArgs.raw` uses `#[arg(long)]` only (no short alias) | Reserves `-r` for a future `--region` override on `show`. | No shadowing of a likely-future flag. |
| Pure `collect_for_display` tests are NOT `#[serial]` | They don't touch process env or filesystem, so serializing them is needless throughput cost. The 4 `execute(...)` tests that hit env+CWD remain `#[serial]`. | Two parallel-safe tests + four serial tests. |
| `field_summary` uses `-` for unset fields in text mode | Concise tabular layout; users can `--format json` if they need to distinguish "unset" from "literal `-`". | Text output stays scannable. |

## Deviations from Plan

None — plan executed as written. Three minor implementation refinements that the plan body anticipated but are worth recording:

1. **Plan body's `print_json` builds an `active_source_serializable` clone** of the `&ActiveSource`. Since `ActiveSource` derives `Copy`, this allocation/match was unnecessary; removed. The serialized output is byte-identical.
2. **Plan body's `print_merged_with_attribution` returned `Result<()>`** but never errored. Changed return type to `()` since it's pure formatting + `println!`. `execute` simplified accordingly.
3. **Plan body's `print_raw` wrapper signature** was `BTreeMap<String, &TargetEntry>` (borrowed). As anticipated by the plan note, this hits serde lifetime issues with `#[serde(tag = "type")]` on the wrapped enum; switched to owned clone. This is a Rule 3 fix (blocking — code wouldn't compile otherwise) but the plan body explicitly listed it as the recovery path.

All three are isolated to display code; no behavior change in the data the user sees.

## Issues Encountered

- **Sandbox blocked end-to-end smoke test of the binary.** The plan's `<verification>` section recommended `cargo run -p cargo-pmcp -- configure list --format json | jq .targets`. Building/invoking the binary outside `cargo test` was sandbox-denied. The 12 unit tests exercise every code path including the JSON shape end-to-end (parse round-trip, schema_version, active_source, BTreeMap order all asserted), and `cargo build -p cargo-pmcp --quiet` exits 0, so the binary smoke test is informational only — the unit-level coverage is sufficient evidence of behavior. Operator can re-run the smoke step after merge.
- **Pre-existing pentest/loadtest/deployment clippy errors** carry over from earlier phases. Confirmed `configure/list.rs` and `configure/show.rs` add no new clippy errors (filter `cargo clippy 2>&1 | grep configure/` produces zero matches). Out-of-scope per the SCOPE BOUNDARY rule.

## Verification Results

| Check | Result |
|---|---|
| `cargo build -p cargo-pmcp --quiet` | exit 0 (only pre-existing pentest dead-code warnings) |
| `cargo test -p cargo-pmcp --bins commands::configure::list -- --test-threads=1` | 6/6 passed |
| `cargo test -p cargo-pmcp --bins commands::configure::show -- --test-threads=1` | 6/6 passed |
| `cargo test -p cargo-pmcp --bins commands::configure -- --test-threads=1` (full configure suite) | 40/40 passed (8 add + 8 use_cmd + 10 config + 2 workspace + 6 list + 6 show) |
| `grep -c "pub struct ListArgs" configure/list.rs` | 1 ✓ |
| `grep -c "schema_version: u32" configure/list.rs` | 1 ✓ |
| `grep -c "active_source" configure/list.rs` | 11 ✓ (≥2 required) |
| `grep -c "WorkspaceMarker\|workspace_marker" configure/list.rs` | 4 ✓ (≥2 required) |
| `grep -c "pub struct ShowArgs" configure/show.rs` | 1 ✓ |
| `grep -c "pub raw: bool" configure/show.rs` | 1 ✓ |
| `grep -c "source: target" configure/show.rs` | 4 ✓ (≥1 required for Plan 06 placeholder) |
| `grep -c "api_url     =" configure/show.rs` | 1 ✓ (D-13 fixed-order banner field) |
| `grep -c "aws_profile =" configure/show.rs` | 1 ✓ |
| `grep -c "region      =" configure/show.rs` | 1 ✓ |

## Threat Surface Compliance

The plan's `<threat_model>` flagged 3 threats:

| Threat | Mitigation Result |
|---|---|
| T-77-04 (PMCP_TARGET set to non-existent target makes list confusing) | mitigated — `compute_active_target` returns the env name regardless; text mode emits `note: active target overridden by PMCP_TARGET env var` to stderr; `--format json` exposes `active_source: "env"`. The user sees `active = "prod"` in JSON or `*` on no row in text → easy to spot a stale env override. |
| T-77-04-A (malformed `.pmcp/active-target` crashes list) | mitigated — `compute_active_target` wraps `find_workspace_root` in `Ok(...)` so a non-Cargo CWD becomes "no active marker" rather than failing list. `read_active_marker` (Plan 04) is BOM/whitespace-tolerant and returns `Ok(None)` on missing/empty. |
| T-77-07-A (`--raw` exposes accidentally-pasted secrets despite Plan 04's validator) | accepted — file is `0o600` user-only; raw display is by user request; references-only policy is enforced at `add` time. No new mitigation needed. |

No new threat surface introduced beyond what the plan anticipated.

## Next Phase Readiness

Plan 77-06 (resolver + banner) is unblocked:

- `compute_active_target` (in list.rs) and `resolve_active_or_fail` (in show.rs) both use the canonical env > marker order Plan 06's full resolver will extend with `--target` flag + `deploy.toml` walk.
- `collect_for_display` in show.rs is the stable contract Plan 06's resolver enrichment will plug into: same `(api_url, aws_profile, region, extras)` tuple shape, just replacing every `(source: target)` literal with the resolved source string.
- `ListJsonOutput` / `TargetJson` / `ActiveSource` are stable wire shapes — Plan 07's CLI wiring can rely on `--format json` consumers (scripts, CI gates) to keep parsing the same fields after Plan 06 lands.

Plan 77-06 will:
1. Build the full 4-source resolver (env > flag > target > deploy.toml).
2. Replace the body of `print_merged_with_attribution` to label each field with its real source.
3. Add the OnceLock-guarded `emit_resolved_banner_once` for target-consuming commands (NOT show — RESEARCH Q6).

## Self-Check: PASSED

**Files verified:**

- `[ -f cargo-pmcp/src/commands/configure/list.rs ]` → FOUND (370 lines)
- `[ -f cargo-pmcp/src/commands/configure/show.rs ]` → FOUND (338 lines)

**Commits verified in `git log --oneline`:**

- `27be341a` (Task 1) → FOUND
- `cb4fd522` (Task 2) → FOUND

---
*Phase: 77-cargo-pmcp-configure-commands*
*Plan: 05*
*Completed: 2026-04-26*
