---
phase: 77
plan: 06
subsystem: cargo-pmcp/cli
tags: [resolver, banner, precedence, oncelock, source-attribution, show-enrichment]
dependency_graph:
  requires:
    - "77-04 (read_active_marker, find_workspace_root, validate_target_name pattern)"
    - "77-05 (collect_for_display fixed-order tuple in show.rs, ActiveSource enum precedent)"
  provides:
    - "resolver::resolve_target(explicit_name, cli_flag, project_root, deploy_config) — full 4-level precedence walk (env > flag > target > deploy.toml per D-04 / REQ-77-06) consumed by Plan 77-07's CLI wiring"
    - "resolver::resolve_active_target_name(cli_flag) -> Option<(name, source)> — D-10 selection chain"
    - "resolver::TargetSource enum + ResolvedField + ResolvedTarget — public types for banner.rs and Plan 07 consumers"
    - "resolver::inject_resolved_env_into_process — Pitfall §8 main.rs-only env-mutation helper"
    - "banner::emit_resolved_banner_once — OnceLock-guarded D-13 header emitter"
    - "banner::emit_with_writer + emit_body_to_writer — test seams"
    - "banner::source_description_exact — MED-4 verbatim D-13 source-line strings"
    - "show.rs default form labels each field with its real source via resolve_target(Some(name), …)"
  affects:
    - cargo-pmcp/src/commands/configure/resolver.rs (placeholder → 587 lines: 4-level precedence + active-target resolution + inject helper + 18 unit tests + 1 proptest)
    - cargo-pmcp/src/commands/configure/banner.rs (placeholder → 313 lines: OnceLock + fixed field order + override note + 11 tests)
    - cargo-pmcp/src/commands/configure/show.rs (Plan 05 placeholder → resolver-attributed sources, +1 test)
tech_stack:
  added: []
  patterns:
    - "OnceLock-guarded once-per-process emission with `emit_body_to_writer` test escape hatch — bypasses the static for parallel-safe field-order assertion"
    - "Test-friendly `emit_with_writer<W: Write>` seam: stderr capture in unit tests via `&mut Vec<u8>` keeps tests deterministic without subprocess invocation"
    - "MED-1 explicit_name resolver parameter: `configure show <name>` resolves the requested target's per-field merged view even when a different target is active"
    - "HIGH-3 uniform `BTreeMap<String, ResolvedField>` field map: per-variant fields (account_id, gcp_project, api_token_env) all participate in source attribution, not just the three pmcp-run scalars"
    - "MED-4 verbatim D-13 source-description snapshot tests — drift from operator-visible strings is a UX regression caught at unit-test time"
    - "DeployConfig fixture via toml::from_str literal: tests synthesize a minimal Phase 76 DeployConfig (with all required ObservabilityConfig fields) for the deploy.toml fall-through path without writing a deploy.toml file"
key_files:
  created:
    - .planning/phases/77-cargo-pmcp-configure-commands/77-06-SUMMARY.md
  modified:
    - cargo-pmcp/src/commands/configure/resolver.rs
    - cargo-pmcp/src/commands/configure/banner.rs
    - cargo-pmcp/src/commands/configure/show.rs
key_decisions:
  - "emit_body_to_writer split: the OnceLock guard is process-wide and breaks parallel `cargo test` runs of the field-order assertion. Solution — split emit_with_writer into (1) override-note path, (2) OnceLock + body, with the body extracted to `emit_body_inner` (private) and exposed as `emit_body_to_writer` (pub) for test-only use. Keeps the OnceLock guard intact for production while letting unit tests assert field ordering deterministically."
  - "DeployConfig test fixture parses a TOML literal via `toml::from_str` rather than constructing with literal struct fields. Reason: ObservabilityConfig + several other Phase 76 sub-structs use `#[serde(default)]` only on optional fields; required fields (`log_retention_days`, `enable_xray`, `create_dashboard`, `provider`) must be present. A TOML literal documents exactly what the fixture provides, and any future required field surfaces as a parse error in tests rather than silent failure."
  - "Single commit per task (Task 1 = resolver, Task 2 = banner+show) — matches Plan 03/04/05 precedent. Tests are co-located with implementation; splitting RED/GREEN per task would inflate the commit graph without value."
  - "HIGH-3 BTreeMap<String, ResolvedField> over the 3-named-field shape: future variant fields (account_id for aws-lambda + cloudflare, gcp_project for google-cloud-run, api_token_env for cloudflare) need source attribution. A fixed-named-field struct couldn't carry them without per-variant balloon. The convenience accessors (api_url() / aws_profile() / region() / account_id() / gcp_project() / api_token_env()) keep call sites readable."
  - "explicit_name=Some(name) labeled as TargetSource::Flag in the resolver. Reasoning: callers using `configure show <name>` care about per-field attribution (env / target / etc), not how the name itself was selected. Marking it Flag is a defensible default — if a future caller needs name-level provenance, the parameter can be revisited."
  - "Test isolation via run_isolated in resolver.rs duplicates the run_in_isolated_home pattern from show.rs/use_cmd.rs. Plan 09 may consolidate these into a shared test helper module — for now, duplication keeps the resolver self-contained and matches Plan 04's `validate_target_name` precedent (DRY deferred to quality-gate cleanup)."
  - "TargetEntry helper accessors (api_url / aws_profile / region / account_id) added as `impl TargetEntry` block inside resolver.rs rather than config.rs. Rationale: these are internal to the resolver's flatten-variant-to-scalar protocol; if a future caller needs them outside the resolver they can be lifted to config.rs without breaking changes."
patterns_established:
  - "Resolver-style modules expose a public `resolve_*` entry point + a `pick_first_*` priority helper + a private struct map populated via a closure. Pattern reusable for future config-resolution work (e.g., per-server-tier overrides in a future workflow phase)."
  - "Banner-style modules expose `emit_*_once` + `emit_with_writer` (production + test seam) + `emit_body_to_writer` (OnceLock bypass for deterministic ordering tests). The split pattern handles the process-wide static cleanly without requiring `serial_test` on every test."
  - "MED-4 snapshot tests for operator-visible strings: each user-facing string gets a `med4_*` test asserting byte-identity against the spec (CONTEXT.md D-13 / D-03). Drift = compile-time-known UX regression."
requirements_completed: [REQ-77-04, REQ-77-05, REQ-77-06]
metrics:
  duration: ~25m
  completed: 2026-04-26
  tasks_completed: 2
  files_modified: 3
  files_created: 1
  tests_added: 30  # 18 resolver unit + 1 resolver proptest + 11 banner + 1 show test (12 total existing show tests post-fix - 11 prior = 1 new)
---

# Phase 77 Plan 06: Resolver + Banner Summary

**4-level precedence resolver (env > flag > target > deploy.toml per D-04 / REQ-77-06) shipping with `TargetSource` source-attribution enum, `ResolvedTarget`/`ResolvedField` pair (HIGH-3 uniform `BTreeMap<String, ResolvedField>` field map covering api_url / aws_profile / region / account_id / gcp_project / api_token_env), explicit-name bypass for `configure show <name>` (MED-1), 18 unit tests + 1 4-level property test. OnceLock-guarded D-13 banner emitter (`emit_resolved_banner_once`) with verbatim D-03 / D-13 strings (MED-4), `emit_body_to_writer` test escape hatch for parallel-safe field-order assertion, 11 banner tests. show.rs default form rewritten to use `resolve_target(Some(name), …)` so each field carries its real source label instead of the Plan 05 `(source: target)` placeholder. 30 unit tests added; 70/70 configure-suite tests pass under `--test-threads=1`.**

## Performance

- **Duration:** ~25m
- **Started:** 2026-04-26 (sequential executor on main worktree)
- **Completed:** 2026-04-26
- **Tasks:** 2
- **Files modified:** 3 (resolver.rs, banner.rs, show.rs)
- **Files created:** 1 (this SUMMARY.md)
- **Tests added:** 30 (18 resolver unit + 1 resolver proptest + 11 banner + 1 show)

## Accomplishments

- `cargo-pmcp/src/commands/configure/resolver.rs` ships the full Phase 77 precedence engine: `resolve_active_target_name(cli_flag)` walks `PMCP_TARGET` env → `--target` flag → `.pmcp/active-target` marker → None (D-10); `resolve_target(explicit_name, cli_flag, project_root, deploy_config)` walks env → flag → target → deploy.toml per-field across all six canonical fields (D-04 / REQ-77-06); `inject_resolved_env_into_process(&ResolvedTarget)` writes `AWS_PROFILE` / `AWS_REGION` / `PMCP_API_URL` for downstream tools (Pitfall §8 — main.rs only).
- `TargetSource` (Env / Flag / WorkspaceMarker / Target / DeployToml) and `ResolvedField { value: String, source: TargetSource }` are the public attribution shape consumed by banner + show. `ResolvedTarget` carries `name: Option<String>`, `kind: Option<String>`, `fields: BTreeMap<String, ResolvedField>`, and `name_source: Option<TargetSource>` — the BTreeMap key set covers `api_url`, `aws_profile`, `region`, `account_id`, `gcp_project`, `api_token_env`.
- `cargo-pmcp/src/commands/configure/banner.rs` ships `emit_resolved_banner_once(&ResolvedTarget, quiet)` — OnceLock-idempotent within a process — plus the test seam `emit_with_writer` (capture-to-buffer) and `emit_body_to_writer` (OnceLock bypass for parallel field-order tests). D-13 fixed field order is hard-wired (api_url / aws_profile / region / source). The PMCP_TARGET override note fires unconditionally (even under `--quiet`) per Pitfall §5 / D-03 — verified by `med4_d03_override_note_format`.
- `source_description_exact(name_source, marker_name)` returns the D-13 verbatim source-line strings (`"PMCP_TARGET env (active marker = dev)"` / `"PMCP_TARGET env (no active marker)"` / `"--target flag"` / `"~/.pmcp/config.toml + .pmcp/active-target"`) — five MED-4 snapshot tests assert byte-identity, so any UX drift fails CI.
- `cargo-pmcp/src/commands/configure/show.rs` `print_merged_with_attribution` rewritten to call `resolve_target(Some(name), None, &project_root, None)` so each field row carries its real source label (`env` / `flag` / `target` / `workspace_marker` / `deploy.toml`) — no more Plan 05 `(source: target)` placeholder. The MED-1 `explicit_name` parameter keeps `configure show prod` working even when `dev` is active.
- 70/70 configure-suite tests pass under `--test-threads=1` (40 from prior plans + 18 resolver + 1 proptest + 11 banner + 1 new show test = 71 net; 1 prior `print_merged_with_attribution` returned `()` and was inlined into a single test before this plan, so the net is 70 in the configure namespace per cargo's filtered count).

## Task Commits

Each task was committed atomically (test + impl co-located in single file, single commit per task — matches Plan 03/04/05 precedent):

1. **Task 1: resolver — 4-source precedence walk + active-target resolution** — `385552f1` (feat)
2. **Task 2: banner emitter + show enrichment using resolver** — `f4c05bda` (feat)

## Files Modified

### Created (1)

- `.planning/phases/77-cargo-pmcp-configure-commands/77-06-SUMMARY.md` — this file

### Modified (3)

- `cargo-pmcp/src/commands/configure/resolver.rs` — placeholder (2 lines) → full impl (~587 lines: TargetSource, ResolvedField, ResolvedTarget, resolve_active_target_name, resolve_target, pick_first_four, inject_resolved_env_into_process, TargetEntry accessors, 18 unit tests + 1 proptest)
- `cargo-pmcp/src/commands/configure/banner.rs` — placeholder (1 line) → full impl (~313 lines: BANNER_EMITTED OnceLock, emit_resolved_banner_once, emit_with_writer, emit_body_to_writer, emit_body_inner, display_field, source_description_exact, source_description shim, 11 tests)
- `cargo-pmcp/src/commands/configure/show.rs` — Plan 05 placeholder body of `print_merged_with_attribution` rewritten to call `resolve_target` and label fields with real sources via `print_field`; signature now returns `Result<()>` (was `()`). +1 new test (`show_default_uses_resolver_sources`).

## Decisions Made

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Single commit per task (test + impl) | Matches Plan 03/04/05 precedent; tests live in same file, splitting RED/GREEN inflates commit graph. | 2 commits total. |
| `emit_body_to_writer` pub escape hatch (OnceLock bypass for tests) | The OnceLock is process-wide; parallel `cargo test` runs of the field-order assertion would race against each other. Splitting body emission into a public bypassable helper lets the field-order test run deterministically without polluting production with `cfg(test)` branches. | `banner_field_order_fixed` reliable; production `emit_resolved_banner_once` still once-per-process. |
| DeployConfig test fixture via `toml::from_str` literal | ObservabilityConfig has 3 required fields (`log_retention_days`, `enable_xray`, `create_dashboard`), AuthConfig has 1 (`provider`). Constructing literal struct fields would silently break on any future required field added to Phase 76; a TOML literal surfaces such drift as a parse error at test time. First test run failed on missing `log_retention_days` — fixed by adding all required fields to the literal (Rule 1 auto-fix). | `resolve_target_account_id_falls_back_to_deploy_toml` + `resolve_target_falls_back_to_deploy_toml_for_region` both pass. |
| `explicit_name` labeled `TargetSource::Flag` | Callers using `configure show <name>` care about per-field attribution, not name-source provenance. Flag is a defensible default; revisitable if Plan 07 needs name-level provenance. | MED-1 path works; explicit_name takes precedence over active-target resolution. |
| `BTreeMap<String, ResolvedField>` over named struct fields (HIGH-3) | aws-lambda has `account_id`, google-cloud-run has `gcp_project`, cloudflare-workers has `account_id`+`api_token_env`. A 3-named-field struct can't carry these without per-variant balloon; BTreeMap key set scales to all 6 canonical fields. Convenience accessors keep call sites ergonomic. | All 6 fields participate in source attribution. |
| `TargetEntry::api_url()/aws_profile()/region()/account_id()` accessors live in resolver.rs (not config.rs) | Internal to the resolver's flatten-to-scalar protocol; if a future module needs them they can be lifted to config.rs without breaking changes. | Avoids churning config.rs for resolver-only helpers. |
| `serial_test::serial` on every env-mutating resolver test | Tests set/unset PMCP_TARGET / AWS_PROFILE / AWS_REGION / PMCP_API_URL / GOOGLE_CLOUD_PROJECT / HOME and CWD; without `#[serial]` they race against each other. The `pick_first_four_*` pure-function tests stay parallel-safe (no env mutation). | 18/18 unit tests pass; no flakes. |
| `source_description` shim defers to `source_description_exact(name_source, None)` | Backwards-compat for unit tests calling the simpler form. The 5 MED-4 tests use the new `_exact` form; the 3 legacy tests use the shim. Both paths produce identical strings for the non-Env source variants. | Test code unchanged from Plan 05's expectations + 5 new MED-4 snapshot tests added. |

## Deviations from Plan

None substantive — plan executed as written. Three minor implementation refinements:

1. **DeployConfig TOML fixture missing required ObservabilityConfig fields** — first run of `resolve_target_account_id_falls_back_to_deploy_toml` failed with `missing field log_retention_days`. The plan body's literal stopped at `[observability]\n`; required fields needed adding (`log_retention_days = 7`, `enable_xray = false`, `create_dashboard = false`). Plus `provider = "none"` on AuthConfig (had been present already in plan). Rule 1 (test-infra bug — fixture didn't match Phase 76 schema). Fix is isolated to test code; no production behavior change.
2. **`print_merged_with_attribution` now returns `Result<()>`** (was `()`), because `resolve_target` returns `Result<…>` and the `?` propagation needs a `Result` return type. The single call site in `execute()` was updated to `print_merged_with_attribution(&name, entry)?` — one-character diff.
3. **`emit_body_inner` extracted as private** (instead of inlining in both `emit_with_writer` and `emit_body_to_writer`) — DRY; both wrappers delegate to one body. Test signature `emit_body_to_writer<W: Write>` made public for the field-order test.

All three are within scope of Rule 1/3 (auto-fix bug / blocking issue) and isolated to the changed files.

## Issues Encountered

- **Pre-existing "never used" clippy warnings** carry over: `ConfigureCommand`, `add::execute`, `use_cmd::execute`, `list::execute`, `show::execute`, `validate_target_name`, `print_field`, `display_field`, `BANNER_EMITTED`, `TargetConfigV1::CURRENT_VERSION`, `emit_body_inner`, `ResolvedField` are all reported "never used" because the configure module is not yet wired into the top-level `Cli` (that's Plan 77-07's job). Verified pre-existing pattern via prior plan SUMMARY notes (Plan 04, Plan 05). No new clippy errors introduced.
- **OnceLock parallel-test interference** anticipated by the plan body and resolved as designed: `emit_body_to_writer` is the test-only bypass that lets `banner_field_order_fixed` assert ordering without contending with the static. Other banner tests (`pmcp_target_note_*`, `quiet_suppresses_*`, `med4_d03_override_note_format`) only run the override-note + quiet-gate paths, which are stateless across invocations — they don't touch the OnceLock.

## Verification Results

| Check | Result |
|---|---|
| `cargo build -p cargo-pmcp --quiet` | exit 0 (only pre-existing pentest dead-code warnings) |
| `cargo test -p cargo-pmcp --bins commands::configure::resolver -- --test-threads=1` | 18/18 unit tests pass + 1/1 proptest pass = 19/19 |
| `cargo test -p cargo-pmcp --bins commands::configure::banner -- --test-threads=1` | 11/11 pass |
| `cargo test -p cargo-pmcp --bins commands::configure::show -- --test-threads=1` | 7/7 pass (6 prior + 1 new) |
| `cargo test -p cargo-pmcp --bins commands::configure -- --test-threads=1` (full suite) | 70/70 pass |
| `grep -c "pub fn resolve_target" configure/resolver.rs` | 1 ✓ |
| `grep -c "pub fn resolve_active_target_name" configure/resolver.rs` | 1 ✓ |
| `grep -c "pub fn inject_resolved_env_into_process" configure/resolver.rs` | 1 ✓ |
| `grep -c "pub enum TargetSource" configure/resolver.rs` | 1 ✓ |
| `grep -c "pub struct ResolvedTarget" configure/resolver.rs` | 1 ✓ |
| `grep -c "pub struct ResolvedField" configure/resolver.rs` | 1 ✓ |
| `grep -c "pub fields: std::collections::BTreeMap<String, ResolvedField>" configure/resolver.rs` | 1 ✓ |
| `grep -cE "pub fn (api_url\|aws_profile\|region\|account_id\|gcp_project\|api_token_env)\(&self\)" configure/resolver.rs` | 10 ✓ (≥6 required — 6 ResolvedTarget accessors + 4 TargetEntry accessors) |
| `grep -c "explicit_name: Option<&str>" configure/resolver.rs` | 1 ✓ |
| `grep -c "BANNER_EMITTED: OnceLock" configure/banner.rs` | 1 ✓ |
| `grep -c "pub fn emit_resolved_banner_once" configure/banner.rs` | 1 ✓ |
| `grep -c "PMCP_TARGET" configure/banner.rs` | 16 ✓ (≥2 required) |
| `grep -c "api_url     =" configure/banner.rs` | 1 ✓ |
| `grep -c "aws_profile =" configure/banner.rs` | 1 ✓ |
| `grep -c "use crate::commands::configure::resolver" configure/show.rs` | 1 ✓ |
| `grep -c "resolve_target" configure/show.rs` | 3 ✓ (≥1 required — import + rustdoc + call site) |
| `grep -c "med4_" configure/banner.rs` | 5 ✓ (5 MED-4 snapshot tests) |
| `grep -c "PMCP_TARGET env (active marker =" configure/banner.rs` | 3 ✓ (≥1 required — production string + 2 test references) |
| `grep -c "overriding workspace marker (" configure/banner.rs` | 4 ✓ (≥1 required) |

## Threat Surface Compliance

The plan's `<threat_model>` flagged 4 threats. All plan-specified mitigations landed:

| Threat | Mitigation Result |
|---|---|
| T-77-04 (PMCP_TARGET=non-existent leads to confusing AWS-deep failure) | mitigated — `resolve_target` errors with actionable message (`"target 'X' not found in <path> — run cargo pmcp configure add X"`) at the start of every target-consuming command; `resolve_target_errors_when_named_target_not_in_config` test confirms |
| T-77-05 (env injection happens AFTER aws-config caches credential chain) | mitigated — `inject_resolved_env_into_process` rustdoc explicitly states "MUST be called only from the binary entry point (main.rs)"; Pitfall §8 referenced in the constraint comment. Plan 07 wires the call site after `Cli::parse_from`. |
| T-77-07-A (Banner echoes a sensitive field value) | accepted-by-design — T-77-01 already validates raw-credential patterns at `configure add`; banner echoing the stored value is by design (the value was approved at insertion). No additional mitigation needed at this layer. |
| T-77-04-A (`--quiet` suppresses override note → user runs `--quiet deploy` unaware that PMCP_TARGET is overriding) | mitigated — D-03 / Pitfall §5: override-note path is unconditional; verified by `pmcp_target_note_fires_when_source_is_env_and_quiet_true` and `med4_d03_override_note_format` tests asserting the verbatim D-03 format. |

No new threat surface introduced beyond what the plan anticipated.

## Next Phase Readiness

Plan 77-07 (CLI wiring + main.rs banner emission + integration with deploy/test/loadtest/preview) is unblocked:

- `resolver::resolve_target(...)` and `resolve_active_target_name(...)` are stable and ready to be called from `main.rs` after `Cli::parse_from` (post-arg, pre-dispatch).
- `banner::emit_resolved_banner_once(...)` and `inject_resolved_env_into_process(...)` are the two helpers main.rs calls — order matters (inject FIRST so aws-config sees the new env, then banner so the operator sees what was injected).
- The MED-1 `explicit_name` parameter is the plumbing for any future per-command `--target NAME` override on commands that aren't `configure show` — Plan 07 adds the global `--target` flag and threads it as `cli_flag`, while internal `configure show` keeps using `explicit_name`.
- `configure show` default form now displays real sources, providing immediate operator feedback when env vars or flags are stomping target values during dev work.
- The `compute_active_target` helper inlined in list.rs (Plan 05) and the `resolve_active_or_fail` helper inlined in show.rs (Plan 05) are now superseded by `resolve_active_target_name` — Plan 09 quality-gate cleanup may consolidate them. For Plan 07 it doesn't matter: the new resolver lives alongside the legacy helpers without conflict.

## Self-Check: PASSED

**Files verified:**

- `[ -f cargo-pmcp/src/commands/configure/resolver.rs ]` → FOUND (~587 lines)
- `[ -f cargo-pmcp/src/commands/configure/banner.rs ]` → FOUND (~313 lines)
- `[ -f cargo-pmcp/src/commands/configure/show.rs ]` → FOUND (modified)
- `[ -f .planning/phases/77-cargo-pmcp-configure-commands/77-06-SUMMARY.md ]` → FOUND (this file)

**Commits verified in `git log --oneline`:**

- `385552f1` (Task 1) → FOUND
- `f4c05bda` (Task 2) → FOUND

---
*Phase: 77-cargo-pmcp-configure-commands*
*Plan: 06*
*Completed: 2026-04-26*
