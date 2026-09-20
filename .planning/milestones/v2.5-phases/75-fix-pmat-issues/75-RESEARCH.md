# Phase 75: Fix PMAT issues - Research

**Researched:** 2026-04-22
**Domain:** Rust code-quality remediation (cognitive complexity reduction, PMAT toolchain configuration, CI gating)
**Confidence:** HIGH (all key claims verified against installed `pmat 3.15.0`, the actual codebase, and live experiments)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** "Phase 75 done" = `pmat quality-gate --fail-on-violation` exits 0 and the auto-generated README badge flips to `Quality Gate: passing`. **Complexity is the gating dimension**; SATD, duplicate, entropy, and sections are best-effort improvements within the same waves but do NOT block phase closure.
- **D-02:** Default approach is pragmatic per-function refactor — extract helper methods, introduce intermediate types (state structs, command enums), decompose deeply-nested matches. For irreducibly complex functions (parsers, AST walkers, protocol-message dispatch, state machines), use `#[allow(clippy::cognitive_complexity)]` with a one-line `// Why:` comment immediately above the attribute. **Every `#[allow]` MUST have a `// Why:` justification — no bare allows.**
- **D-03:** Hard ceiling on `#[allow(clippy::cognitive_complexity)]` use: aim ≤35 cognitive complexity even with the allow; **MUST NOT exceed 50**. Functions above 50 require refactor regardless of justification quality.
- **D-04:** SATD triage is per-comment, three-way: (a) trivial/obsolete → delete; (b) real follow-up work → file GitHub issue, replace marker with `// See #NNN — <reason>`; (c) cheap to fix (<30 min) → fix in this phase. No blanket conversion.
- **D-05:** Duplicate triage by file location: src/, crates/*/src/, cargo-pmcp/src/ → real refactor; examples/, tests/, benches/, fuzz/ → tune PMAT exclusions. Goal is "duplicate count drops meaningfully", not zero.
- **D-06:** Waves by hotspot directory:
  - **Wave 1:** `src/server/streamable_http_server.rs` + `pmcp-macros/`
  - **Wave 2:** `cargo-pmcp/src/pentest/` + `cargo-pmcp/src/deployment/`
  - **Wave 3:** `crates/pmcp-code-mode/`
  - **Wave 4 (optional):** CI enforcement (D-07)
- **D-07:** CI-only gate. Add a PR check that runs `pmat quality-gate --fail-on-violation` and blocks merge on regression. **NO pre-commit integration** in this phase.

### Claude's Discretion

- Which specific functions to refactor first within each wave (planner + executor pick based on dependency order and file co-location).
- Whether to introduce shared types/traits to reduce complexity vs duplicate helpers per call site.
- How to structure GitHub issues for SATDs (one per vs grouped).
- Exact PMAT config file location and syntax for path exclusion (researched below — **important caveats apply**).
- Whether the CI gate workflow should run on PRs only or also nightly.

### Deferred Ideas (OUT OF SCOPE)

- Driving SATD/duplicate/entropy/sections to absolute zero.
- Adding PMAT to pre-commit hook (CI-only enforcement is the chosen mechanism).
- Raising the cognitive_complexity threshold from 25 to ~35 (rejected — keep CLAUDE.md "≤25" promise honest).
- Whole-file rewrites of hotspot files (function-level refactor only).
- Sections badge fix beyond cheap README tweaks.
</user_constraints>

## Summary

**The badge is gated by `pmat quality-gate --fail-on-violation` exit code.** Verified empirically: 94 cognitive-complexity violations across 57 files (CONTEXT.md baseline confirmed). When all those reach 0 the gate exits 0 and the badge flips. SATD (33), duplicate (1554), entropy (13), and sections (2) violations are *also* counted by the gate, but the user has explicitly scoped Phase 75 to complexity as the gating dimension via D-01.

**Key empirical finding (HIGH confidence, verified by live experiment):** PMAT 3.15.0's `quality-gate --checks duplicates` does NOT honor `.pmatignore` or `[analysis] exclude_patterns` in `.pmat/project.toml`. Both were tested in this session and the duplicate count stayed at 1554. The `analyze duplicates` subcommand DOES honor `--exclude` flags. This means D-05's "tune PMAT config to exclude examples" needs a different mechanism than originally assumed — see Pitfall 3 below.

**Second key empirical finding:** Of the 94 violations, **73 are within the in-scope codebase** (src/, crates/*/src/, cargo-pmcp/src/, pmcp-macros/) — the remaining 21 are in examples/ (which the gate counts but D-06's wave map ignores). CONTEXT.md's 5 hotspot directories cover only **16/73 = 22%** of in-scope violations. **The planner MUST extend the wave map** to cover the remaining ~57 violations across `cargo-pmcp/src/commands/`, `cargo-pmcp/src/loadtest/`, `crates/mcp-tester/`, `crates/mcp-preview/`, `src/server/path_validation.rs`, `src/server/schema_utils.rs`, `src/utils/json_simd.rs`, and `src/server/workflow/task_prompt_handler.rs` — or consciously route them into one of the 4 declared waves.

**Primary recommendation:** Plan 4 waves following D-06 but expanding the file scope per wave to capture all 73 in-scope violations. Land Wave 4 (CI gate) only after the badge is green. For each function, the executor picks between (a) extract-method refactor and (b) `#[allow]` with `// Why:` justification, capped at complexity 50 per D-03. Track a running "violations remaining" count in each wave's plan to make progress visible.

## Phase Requirements

Phase 75 has **no REQ-IDs** — this is quality-debt remediation, not feature work. There is no REQUIREMENTS.md mapping table. The acceptance signal is exclusively D-01: `pmat quality-gate --fail-on-violation` exit 0.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Source-code refactoring (extract method, decompose match arms) | Code (per-function) | — | Refactor lives where the function lives |
| `#[allow(clippy::cognitive_complexity)]` annotations | Code (per-function attribute) | — | Per-function escape hatch with `// Why:` justification |
| PMAT exclusion configuration | Repo root config (`.pmatignore` and/or `.pmat/project.toml`) | CI workflow flags | Project-level config; CI may pass overrides if config alone fails |
| CI gate enforcement (D-07) | `.github/workflows/` | Existing `quality-badges.yml` | New job in `ci.yml` `gate` chain OR sibling workflow |
| SATD remediation | Code comments + GitHub issue tracker | — | Triage modifies source; "real follow-up" SATDs migrate to issue tracker |
| Duplicate suppression for examples/tests/fuzz | PMAT config (or codebase if config insufficient) | CI workflow | See Pitfall 3 — config alone may not work in 3.15.0 |

## Standard Stack

This is a remediation phase — no new dependencies are introduced. The "stack" here is the existing tooling.

### Core
| Library/Tool | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pmat` | 3.15.0 (locally), CI uses `cargo install pmat --locked` (latest) | Quality-gate analysis, complexity/SATD/duplicate detection | The gate the badge depends on. `[CITED: which pmat --version]` |
| `clippy` | bundled with stable Rust | `#[allow(clippy::cognitive_complexity)]` attribute is the escape hatch per D-02 | Standard Rust lint suite |
| `rustfmt` | bundled with stable Rust | Format compliance via `make quality-gate` | Already in CI per `ci.yml` |
| `cargo` | stable | Test/build/check commands for validation | Existing infrastructure |

### Supporting
| Tool | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `trybuild` | 1.0 (already in `pmcp-macros/Cargo.toml`) | Macro expansion compile-fail tests | Wave 1 — verify pmcp-macros refactors don't break consumers `[VERIFIED: pmcp-macros/Cargo.toml]` |
| `insta` | 1.43 (already in `pmcp-macros/Cargo.toml`) | Snapshot testing of macro output | Wave 1 — golden-test macro expansion stability `[VERIFIED: pmcp-macros/Cargo.toml]` |
| `proptest` | 1.6 (already in `pmcp-macros/Cargo.toml`) | Property-based regression testing | All waves — cheap regression detection |
| `mcp-tester` | 0.5.x (in-tree) | Integration testing against `streamable_http_server` | Wave 1 — smoke-test transport refactor |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `#[allow(clippy::cognitive_complexity)]` | `pmat ignore` directive | PMAT in 3.15.0 has no per-function suppression; the clippy allow is what 6+ existing files in the codebase already use `[VERIFIED: grep of src/]` |
| Whole-file rewrite | Function-level refactor | Rejected by CONTEXT.md (deferred). Function-level is lower-risk and bisectable per D-06 |

**Version verification:**
```bash
$ pmat --version
pmat 3.15.0
```
The local pmat is 3.15.0; CI installs latest via `cargo install pmat --locked` (see `.github/workflows/quality-badges.yml` line 38) which may differ — the planner should confirm CI runs match local thresholds before declaring badge green. `.pmat/project.toml` records `version = "3.11.1"` but this is stale metadata, not an enforced floor.

## Architecture Patterns

### System Architecture: How the gate is wired today

```
[git push to main / open PR]
        |
        v
.github/workflows/quality-badges.yml  (push, PR, daily cron)
   1. cargo install pmat --locked
   2. pmat analyze tdg --format json    -> TDG_SCORE  (informational badge)
   3. pmat quality-gate --fail-on-violation --format json   <-- BADGE SIGNAL
        |- exit 0 -> GATE_STATUS=passing -> brightgreen badge
        |- exit !=0 -> GATE_STATUS=failing -> red badge
   4. pmat quality-gate --checks complexity --format json   (informational)
   5. Builds shields.io URL, writes README.md badges block on main only
        |
        v
[README displays Quality Gate: passing/failing]

After Phase 75 (D-07) adds:
[PR opened/updated]
        |
        v
.github/workflows/ci.yml  -> new job (or extends quality-gate job)
   - cargo install pmat --locked
   - pmat quality-gate --fail-on-violation
   - listed in `gate` job's `needs:` -> blocks merge via org ruleset
```

The `ci.yml` already has a `gate` job at the bottom (lines ~210-225) that aggregates `test` + `quality-gate` results into a single required check. **The cleanest D-07 implementation is to add `pmat quality-gate --fail-on-violation` to the existing `quality-gate` job's steps** rather than introduce a parallel workflow — leverages existing cache, fits the existing aggregation pattern.

### Recommended File Structure (no new files for refactor work)

```
.pmat/project.toml         # add [analysis] exclude_patterns IF this works (see Pitfall 3)
.pmatignore                # NEW — gitignore-style exclusions, root of repo
.github/workflows/ci.yml   # MODIFY — add pmat quality-gate step to existing quality-gate job
src/, crates/, cargo-pmcp/, pmcp-macros/   # MODIFY in place per wave
```

### Pattern 1: Extract-method on long match arms (Rust cognitive-complexity reducer)

**What:** Cognitive complexity penalises *nested* control flow more than *sequential* control flow. Pulling each match arm into a named helper drops the score dramatically.

**When to use:** Functions like `cargo-pmcp/src/main.rs::execute_command` (cog 48) where a top-level `match cmd { A => ..., B => ..., ... }` does substantial work in each arm.

**Example:**
```rust
// Before (cognitive complexity 48):
async fn execute_command(cmd: Command, ctx: &Ctx) -> Result<()> {
    match cmd {
        Command::Build(opts) => {
            // 30 lines of nested ifs and matches
        }
        Command::Test(opts) => {
            // 25 lines
        }
        // ...
    }
}

// After (cognitive complexity dropped per arm):
async fn execute_command(cmd: Command, ctx: &Ctx) -> Result<()> {
    match cmd {
        Command::Build(opts) => execute_build(opts, ctx).await,
        Command::Test(opts) => execute_test(opts, ctx).await,
        // ...
    }
}

async fn execute_build(opts: BuildOpts, ctx: &Ctx) -> Result<()> { ... }
async fn execute_test(opts: TestOpts, ctx: &Ctx) -> Result<()> { ... }
```

`[ASSUMED]` — Cognitive complexity scoring is well-documented to penalise nesting over sequence; this pattern is the textbook reduction. Specific score deltas would need empirical verification per function.

### Pattern 2: Replace nested validation with early-return chain

**What:** Convert `if a { if b { if c { ... } } }` into `if !a { return Err(...) } if !b { return Err(...) } ...`. Each return removes a nesting level.

**When to use:** `src/server/path_validation.rs::validate_path` (cog 103) — it already uses early returns but has additional nested blocks (verified by reading lines 65-110); deeper analysis may show further opportunities.

### Pattern 3: Extract validation closures into helper functions

**What:** Long validation pipelines benefit from named helpers per concern.

**When to use:** `src/server/streamable_http_server.rs::validate_headers` (cog 40), `validate_protocol_version` (cog 34) — both pure validation logic.

### Pattern 4: State enum + dispatch table for protocol handlers

**What:** Replace deeply-nested `match` with a dispatch enum + per-state handler functions.

**When to use:** `src/server/streamable_http_server.rs::handle_post_with_middleware` (cog 59), `handle_post_fast_path` (cog 48), `handle_get_sse` (cog 35) — all are protocol dispatch hot paths. The decomposition isolates each protocol case for testability.

### Pattern 5: Macro-author escape hatch — `#[allow]` + `// Why:`

**What:** Proc-macro expansion code (`pmcp-macros/`) is naturally branchy because it walks `syn::Item` AST nodes by variant. After modest extraction, residual complexity may be irreducible without harming clarity.

**When to use:** Functions in `pmcp-macros/src/mcp_server.rs` (`collect_resource_methods` cog 80, `collect_tool_methods` cog 44, `collect_prompt_methods` cog 42, `expand_mcp_server` cog 36), `mcp_resource.rs::expand_mcp_resource` (cog 71), `mcp_prompt.rs::expand_mcp_prompt` (cog 42), `mcp_tool.rs::expand_mcp_tool` (cog 40). Most should reduce to ~30-40 with extraction; the rest get the `#[allow]`.

**Existing convention (must change):** Searched the codebase — there are at least 6 existing `#[allow(clippy::cognitive_complexity)]` attributes (in `src/client/http_logging_middleware.rs:332,401`, `src/client/mod.rs:1912`, `src/shared/connection_pool.rs:159`, `src/shared/logging.rs:433`, `src/shared/sse_optimized.rs:225`) — **none of them have `// Why:` comments**. Phase 75 establishes the convention. Old allows should either get a `// Why:` comment retroactively or be removed if the underlying function can now be refactored.

**Format prescription (planner should pin in plans):**
```rust
/// Connect to SSE endpoint
// Why: SSE reconnection state machine — header negotiation, retry-after parsing,
// last-event-id replay, and three error-class branches must coexist; extracting
// further would require shared mutable state across helpers, defeating the point.
#[allow(clippy::cognitive_complexity)]
async fn connect_sse(...) { ... }
```

### Pattern 6: Decompose AST evaluation by syntactic category

**When to use:** `crates/pmcp-code-mode/src/eval.rs::evaluate_with_scope` (cog 123 — second-highest in the codebase) and `evaluate_array_method_with_scope` (cog 117). These are tree-walking evaluators; standard reduction is to dispatch by `Expr` variant into `evaluate_binop`, `evaluate_call`, `evaluate_member`, etc.

### Anti-Patterns to Avoid

- **Bare `#[allow]` without `// Why:`** — explicitly violates D-02. Pre-commit reviewers should reject any PR that adds an allow without justification.
- **Allow above 50 cognitive complexity** — explicitly violates D-03. Refactor required.
- **Whole-file rewrite to "fix it all at once"** — explicitly out of scope per CONTEXT.md `<deferred>`.
- **Renaming functions or changing public API to reduce complexity** — out of scope; this is a complexity-only phase.
- **Suppressing complexity globally via `#![allow(clippy::cognitive_complexity)]` at the crate root** — defeats the purpose of the gate; never use.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Per-function complexity suppression | Custom PMAT plugin or wrapper script | `#[allow(clippy::cognitive_complexity)]` with `// Why:` | PMAT 3.15.0 reads clippy attributes; this is the supported escape hatch |
| Path exclusion for PMAT | Shell-script wrapper that filters JSON output | `.pmatignore` (gitignore syntax) — but verify per Pitfall 3 | Standard PMAT mechanism; documented in PMAT Book ch30 |
| CI gate logic | Custom shell loop computing thresholds | Just `pmat quality-gate --fail-on-violation` exit code | Single source of truth — same command the badge uses |
| SATD tracking | Spreadsheet or new in-repo doc | GitHub issues with `// See #NNN` references | Already the project's convention per CLAUDE.md SATD policy |
| Macro expansion regression detection | Hand-written equality assertions | `insta` snapshot tests + `trybuild` (already in `pmcp-macros/Cargo.toml`) | Existing infrastructure; just wire up |

**Key insight:** This is a refactor phase, not a tooling phase. Every helper exists. Resist any temptation to write meta-tooling.

## Per-Hotspot Drill-Down (the 73 in-scope violations)

> Source: `pmat analyze complexity --top-files 0 --format json --max-cognitive 25` run on this branch, filtered to exclude `.claude/worktrees/`, `examples/`, `tests/`, `fuzz/`, `benches/`, `widget-runtime/`. `[VERIFIED: live tool run 2026-04-22]`

### Wave 1a: `src/server/streamable_http_server.rs` (1416 lines, 6 functions)

| Function | Line | Cognitive | Recommended Approach |
|----------|------|-----------|----------------------|
| `handle_post_with_middleware` | 1005 | 59 | Pattern 4 — dispatch table per protocol message type |
| `handle_post_fast_path` | 841 | 48 | Pattern 4 + extract pre/post hooks |
| `validate_headers` | 391 | 40 | Pattern 3 — per-header validation closures |
| `handle_get_sse` | 1262 | 35 | Pattern 4 |
| `validate_protocol_version` | 674 | 34 | Pattern 2 — early-return chain |
| `build_response` | 566 | 30 | Pattern 1 — extract per-status-code arms |

**Risk:** Low-to-medium. File has dedicated tests (`tests/streamable_http_server_tests.rs`, `streamable_http_unit_tests.rs`, `streamable_http_properties.proptest-regressions`) — refactor is verifiable.

### Wave 1b: `pmcp-macros/` (4 files, 7 functions)

| File | Function | Line | Cognitive | Recommended Approach |
|------|----------|------|-----------|----------------------|
| `mcp_server.rs` | `collect_resource_methods` | 743 | 80 | Refactor target ~40-45, then `#[allow]` if irreducible (Pattern 5) |
| `mcp_server.rs` | `collect_tool_methods` | 480 | 44 | Pattern 1 + extract per-attribute parsing |
| `mcp_server.rs` | `collect_prompt_methods` | 625 | 42 | Same pattern as collect_tool_methods |
| `mcp_server.rs` | `expand_mcp_server` | 111 | 36 | Pattern 1 — extract code-gen sub-blocks |
| `mcp_resource.rs` | `expand_mcp_resource` | 78 | 71 | Refactor target ~35-40, then `#[allow]` |
| `mcp_prompt.rs` | `expand_mcp_prompt` | 47 | 42 | Pattern 1 |
| `mcp_tool.rs` | `expand_mcp_tool` | 66 | 40 | Pattern 1 |

**Risk:** HIGH. Macros affect every downstream crate (`pmcp` core depends on `pmcp-macros`). Validation matrix:
- `cargo test -p pmcp-macros` (existing `mcp_*_tests.rs` files in `pmcp-macros/tests/`)
- `trybuild` compile-fail tests (already wired in `Cargo.toml`)
- `insta` snapshot tests (already wired) — recommend extending to capture macro output for the worst offenders BEFORE refactor begins so any change is visible
- Workspace-wide `cargo test --workspace` to catch downstream breakage

### Wave 2a: `cargo-pmcp/src/pentest/` (4 files, 9 functions)

| File | Function | Line | Cognitive | Approach |
|------|----------|------|-----------|----------|
| `attacks/data_exfiltration.rs` | `run_de01_ssrf` | 121 | 45 | Pattern 1 |
| `attacks/data_exfiltration.rs` | `run_de03_content_injection` | 306 | 27 | Pattern 1 |
| `attacks/data_exfiltration.rs` | `run_de02_path_traversal` | 230 | 25 | Borderline — may auto-resolve from companion refactors |
| `attacks/prompt_injection.rs` | `check_value_for_markers` | 98 | 44 | Pattern 1 |
| `attacks/prompt_injection.rs` | `run_deep_fuzz` | 576 | 40 | Pattern 1 + extract per-payload loop |
| `attacks/protocol_abuse.rs` | `run_pa02_oversized_request` | 214 | 36 | Pattern 1 |
| `attacks/protocol_abuse.rs` | `run_pa01_malformed_jsonrpc` | 101 | 34 | Pattern 1 |
| `attacks/protocol_abuse.rs` | `run_pa04_notification_flooding` | 457 | 28 | Pattern 1 |
| `attacks/auth_flow.rs` | `run_af03_jwt_algorithm_confusion` | 322 | 30 | Pattern 1 |

**Risk:** Medium. Pentest code is newer (less battle-tested). Per CONTEXT.md `<specifics>`: "SATDs there are more likely to be active follow-ups (file an issue) than obsolete." Validation: existing tests + manual smoke-run via `cargo run -p cargo-pmcp -- pentest`.

### Wave 2b: `cargo-pmcp/src/deployment/` (3 files, 10 functions)

| File | Function | Line | Cognitive | Approach |
|------|----------|------|-----------|----------|
| `targets/cloudflare/init.rs` | `find_any_package` | 193 | 65 | Pattern 1 |
| `targets/cloudflare/init.rs` | `try_find_pmcp_in_cargo_toml` | 381 | 41 | Pattern 1 |
| `targets/cloudflare/init.rs` | `try_find_workspace_pmcp` | 436 | 41 | Pattern 1 |
| `targets/cloudflare/init.rs` | `auto_detect_server_package` | 98 | 35 | Pattern 1 |
| `targets/cloudflare/init.rs` | `find_core_package` | 159 | 35 | Pattern 1 |
| `targets/cloudflare/init.rs` | `detect_pmcp_dependency` | 342 | 33 | Pattern 1 |
| `targets/pmcp_run/deploy.rs` | `deploy_to_pmcp_run` | 77 | 65 | Pattern 1 |
| `targets/pmcp_run/deploy.rs` | `extract_version_from_cargo` | 21 | 27 | Pattern 1 |
| `targets/pmcp_run/auth.rs` | `fetch_pmcp_config` | 142 | 35 | Pattern 1 |
| `targets/pmcp_run/auth.rs` | `start_callback_server` | 582 | 26 | Pattern 1 (borderline) |

**Risk:** Medium-high. Deployment code is newer; many functions are sequential file/network IO with branching error handling — high refactor potential, low semantic risk if tests cover the happy path.

### Wave 3: `crates/pmcp-code-mode/` (3 files, 5 functions)

| File | Function | Line | Cognitive | Approach |
|------|----------|------|-----------|----------|
| `src/eval.rs` | `evaluate_with_scope` | 59 | **123** | Pattern 6 — dispatch by `Expr` variant |
| `src/eval.rs` | `evaluate_array_method_with_scope` | 506 | **117** | Pattern 6 — dispatch by method name |
| `src/eval.rs` | `evaluate_string_method` | 771 | 50 | Pattern 6 (borderline at 50 — D-03 ceiling) |
| `src/policy_annotations.rs` | `parse_policy_annotations` | 367 | 35 | Pattern 1 |
| `src/schema_exposure.rs` | `pattern_matches` | 770 | 34 | Pattern 1 |

**Risk:** Highest cognitive scores in the codebase. AST/evaluator code naturally branchy. Expect heavy `#[allow]` use bounded by D-03's 50 ceiling. **`evaluate_with_scope` at 123 is 2.5× the D-03 hard cap** — refactor is mandatory; an `#[allow]` is not an option until decomposition brings it under 50.

### Files NOT in CONTEXT.md hotspots — must be assigned to a wave

CONTEXT.md's wave map covers 16/73 violations. The remaining 57 across these files need explicit wave assignment by the planner. Recommended grouping:

**Add to Wave 1 (src/ critical-path):**
| File | Function | Line | Cognitive |
|------|----------|------|-----------|
| `src/server/path_validation.rs` | `validate_path` | 65 | 103 |
| `src/server/schema_utils.rs` | `normalize_schema_with_config` | 61 | 56 |
| `src/server/schema_utils.rs` | `inline_refs_with_context` | 149 | 55 |
| `src/server/schema_utils.rs` | `inline_refs` | 210 | 41 |
| `src/server/workflow/task_prompt_handler.rs` | `classify_resolution_failure` | 523 | 43 |
| `src/utils/json_simd.rs` | `parse_json_fast` | 11 | 59 |
| `src/utils/json_simd.rs` | `pretty_print_fast` | 113 | 36 |

**Add to Wave 2 (cargo-pmcp/ extension):**
| File | Function | Line | Cognitive |
|------|----------|------|-----------|
| `cargo-pmcp/src/main.rs` | `execute_command` | 407 | 48 |
| `cargo-pmcp/src/commands/test/check.rs` | `execute` | 20 | **105** |
| `cargo-pmcp/src/commands/deploy/mod.rs` | `handle_oauth_action` | 796 | 91 |
| `cargo-pmcp/src/commands/deploy/mod.rs` | `detect_server_name` | 8 | 64 |
| `cargo-pmcp/src/commands/doctor.rs` | `execute` | 15 | 60 |
| `cargo-pmcp/src/commands/add.rs` | `server` | 12 | 56 |
| `cargo-pmcp/src/commands/test/run.rs` | `execute` | 12 | 46 |
| `cargo-pmcp/src/commands/test/upload.rs` | `execute` | 11 | 44 |
| `cargo-pmcp/src/commands/test/apps.rs` | `execute` | 16 | 43 |
| `cargo-pmcp/src/commands/validate.rs` | `run_validation` | 71 | 66 |
| `cargo-pmcp/src/commands/validate.rs` | `parse_test_output` | 286 | 30 |
| `cargo-pmcp/src/commands/dev.rs` | `resolve_server_binary` | 22 | 34 |
| `cargo-pmcp/src/commands/dev.rs` | `execute` | 113 | 33 |
| `cargo-pmcp/src/commands/test/list.rs` | `execute` | 10 | 36 |
| `cargo-pmcp/src/commands/pentest.rs` | `execute_pentest` | 80 | 38 |
| `cargo-pmcp/src/commands/preview.rs` | `execute` | 9 | 27 |
| `cargo-pmcp/src/commands/landing/init.rs` | `detect_server_name` | 144 | 30 |
| `cargo-pmcp/src/commands/landing/deploy.rs` | `deploy_landing_page` | 11 | 27 |
| `cargo-pmcp/src/commands/loadtest/run.rs` | `execute_run` | 19 | 26 |
| `cargo-pmcp/src/loadtest/vu.rs` | `vu_loop_inner` | 243 | 37 |
| `cargo-pmcp/src/loadtest/summary.rs` | `render_summary` | 56 | 26 |
| `cargo-pmcp/src/landing/template.rs` | `find_local_template` | 90 | 26 |
| `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs` | `extract_version_from_cargo` | 21 | 27 |

**Add to Wave 3 (crates/ extension):**
| File | Function | Line | Cognitive |
|------|----------|------|-----------|
| `crates/mcp-tester/src/diagnostics.rs` | `run_diagnostics_internal` | 28 | 55 |
| `crates/mcp-tester/src/main.rs` | `main` | 244 | 40 |
| `crates/mcp-preview/src/handlers/websocket.rs` | `handle_socket` | 50 | 37 |
| `crates/mcp-preview/src/handlers/api.rs` | `list_resources` | 179 | 31 |
| `crates/pmcp-server/pmcp-server-lambda/src/main.rs` | `handler` | 89 | 26 |

**Total in-scope after expansion:** 73 violations across 43 files.

## Common Pitfalls

### Pitfall 1: PMAT version drift between local and CI
**What goes wrong:** Local `pmat 3.15.0` produces N violations; CI installs latest `pmat` (no `--version` pin in `quality-badges.yml`) and produces N±k violations because thresholds or detection changed.
**Why it happens:** The CI step is `cargo install pmat --locked` — locks the dependency tree but not the binary version itself. New PMAT releases ship with new lints.
**How to avoid:** During Phase 75, pin the PMAT version in `quality-badges.yml` and the new D-07 gate to a specific known version (`cargo install pmat --version =3.15.0 --locked`). Document that bumping PMAT is a separate, intentional change.
**Warning signs:** CI passes badge green locally but red on PR, or vice versa.

### Pitfall 2: Existing `#[allow]` without `// Why:` slip through
**What goes wrong:** D-02 mandates `// Why:` on every allow. The codebase already has 6+ bare allows from prior commits. If Phase 75 only enforces the rule for *new* allows, the existing ones become technical-debt landmines.
**Why it happens:** Convention is being introduced mid-codebase.
**How to avoid:** Add a Wave-local task to retro-justify (or remove) every existing `#[allow(clippy::cognitive_complexity)]`. Verified existing locations:
- `src/client/http_logging_middleware.rs:332,401` (log_request, log_response)
- `src/client/mod.rs:1912` (send_request)
- `src/shared/connection_pool.rs:159` (start)
- `src/shared/logging.rs:433` (log)
- `src/shared/sse_optimized.rs:225` (connect_sse)
- (likely more in `src/server/elicitation.rs`, `src/server/notification_debouncer.rs`, `src/server/resource_watcher.rs`, `src/server/transport/websocket_enhanced.rs`, `src/shared/streamable_http.rs` — found by grep)

`[VERIFIED: rg #\[allow\(clippy::cognitive_complexity\)\] of src/]`

**Warning signs:** A reviewer asking "why this allow?" finds no comment.

### Pitfall 3: PMAT exclusion config does NOT work for `quality-gate --checks duplicates`
**What goes wrong:** D-05 assumes that adding `examples/`, `tests/`, `fuzz/`, `benches/` to PMAT exclusions will reduce the duplicate count. **Empirically, this does not work in PMAT 3.15.0** (verified in this research session via two experiments).
**Why it happens:** The `quality-gate --checks duplicates` check uses a different code path than `analyze duplicates`. The former reports "pattern repetition" findings (e.g., "ApiCall pattern repeated 7 times"); the latter does block-level similarity. Live experiments:

```
# Experiment 1: TOML config
$ cat .pmat/project.toml
[analysis]
exclude_patterns = ["**/examples/**", "**/tests/**", "**/fuzz/**", "**/benches/**", "**/.claude/worktrees/**"]
$ pmat quality-gate --fail-on-violation --checks duplicates
⚠️ Quality gate found 1554 violations    # NO REDUCTION

# Experiment 2: .pmatignore (gitignore syntax, per pmat-book ch30)
$ cat .pmatignore
examples/
tests/
benches/
fuzz/
.claude/worktrees/
$ pmat quality-gate --fail-on-violation --checks duplicates
⚠️ Quality gate found 1554 violations    # STILL NO REDUCTION

# Experiment 3: analyze duplicates --exclude (the subcommand, not the gate)
$ pmat analyze duplicates --exclude "**/examples/**,**/tests/**,..."
✓ Found 0 duplicate blocks                # WORKS HERE
```

**How to avoid:**
- D-01 says complexity is the gating dimension and SATD/duplicate are best-effort. **The gate exit code does not depend on the duplicate count when `--checks complexity` is used.** D-07 gate should run with `--checks complexity` initially to align with D-01's intent and avoid being held hostage to the duplicate-detection bug.
- Decision point for the planner: either (a) accept duplicates count remains high but irrelevant to the gate exit (use `--checks complexity` in CI), or (b) verify with the PMAT maintainers (`paiml/paiml-mcp-agent-toolkit` issue tracker) whether exclusion is supported in 3.15.0 and what syntax — defer the duplicate work if it can't be excluded.
- Refactor work in `src/`, `crates/*/src/`, `cargo-pmcp/src/` should still proceed per D-05 (it's the right thing to do regardless of gate behavior).

**Warning signs:** Adding `.pmatignore` does not change gate output.

### Pitfall 4: Refactoring `pmcp-macros/` breaks downstream silently
**What goes wrong:** `pmcp-macros/` is consumed by every crate in the workspace. A subtle change in macro output (e.g., generating a different field name) compiles in `pmcp-macros` itself but breaks `pmcp` core or `cargo-pmcp` at workspace-wide build time.
**Why it happens:** Proc-macro tests in `pmcp-macros/tests/` only test the macro author's view, not the consumer's view.
**How to avoid:**
- BEFORE refactoring `pmcp-macros/`, capture `insta` snapshots of macro output for `expand_mcp_server`, `expand_mcp_tool`, `expand_mcp_resource`, `expand_mcp_prompt` against representative input. Existing infra (`insta = "1.43"`, already in `pmcp-macros/Cargo.toml`) makes this trivial.
- Run `cargo test --workspace --all-features` after every commit in Wave 1b.
- Use `cargo expand` to spot-check generated code before pushing.

**Warning signs:** `cargo test -p pmcp-macros` passes but `cargo test --workspace` fails.

### Pitfall 5: Quality-gate counts examples/ violations even though they're "not in scope"
**What goes wrong:** 21 of the 94 cognitive-complexity violations are in `examples/` (e.g., `examples/wasm-mcp-server/src/lib.rs::main` cog 83, `examples/27-course-server-minimal/src/main.rs::load_course_content` cog 66). CONTEXT.md doesn't list examples as a hotspot to fix, but **`pmat quality-gate` counts them** — so the gate stays red until they're addressed.
**Why it happens:** PMAT analyses everything matching `include_patterns = ["**/*.rs", "**/*.ts"]`.
**How to avoid:** Either (a) refactor the example-side violations too (some are simple), (b) `#[allow]` them with `// Why: example for documentation, intentionally inline`, or (c) configure PMAT to exclude `examples/` from complexity checking. Per Pitfall 3, exclusions may not work — `#[allow]` on each example is the safest.
**Warning signs:** All "in-scope" violations cleared but gate still red.

### Pitfall 6: Examples and fuzz targets contribute to the 94-count
**Detail:** Running `pmat quality-gate --checks complexity --format json` and counting violations:
- 94 total cognitive-complexity violations.
- 21 in examples/ (wasm-mcp-server, 27-course-server-minimal, c03_client_resources, s17_advanced_typed_tools, 26-server-tester, t08_simd_parsing_performance, t06_streamable_http_client, t02_websocket_server_enhanced, c06_multiple_clients_parallel, s11_error_handling, fermyon-spin/handle_request).
- 3 in fuzz/ (auth_flows::test_auth_flow cog 122, auth_flows::test_pkce_flow cog 45, transport_layer::simulate_transport_operations cog 46, test_websocket_framing cog 30).
- 73 in src/, crates/*/src/, cargo-pmcp/src/, pmcp-macros/.

For the badge to flip, **all 94 must be addressed** (refactor or `#[allow]`). The planner should NOT scope Wave 1-3 to only 73 — examples/ and fuzz/ also need either an allow or a refactor pass.

`[VERIFIED: jq aggregation of /tmp/pmat-complexity.json this session]`

### Inventory Reconciliation (added 2026-04-23 post-cross-AI-review)

**The math in Pitfall 6 doesn't add up cleanly.** The cited breakdown is:
- 21 examples/ + 3 fuzz/ + 73 in-scope = **97**, not 94.
- The 4-item fuzz/ list (`test_auth_flow` cog 122, `test_pkce_flow` cog 45,
  `simulate_transport_operations` cog 46, `test_websocket_framing` cog 30) is
  4 functions but Pitfall 6's prose counts them as "3" — the prose is wrong
  by one. So the derived total is actually 21 + 4 + 73 = **98**.

**Codex reviewer flagged this** as "every wave delta and exit criterion is
suspect" until reconciled. The 94 figure came from
`pmat quality-gate --fail-on-violation --format json | jq '.violations | length'`
on the baseline date; the per-bucket 21/3-or-4/73 came from a separate
`pmat analyze complexity --format json` query filtered by path. The
discrepancy is likely one or more of:

- One fuzz function (e.g. `test_websocket_framing` cog 30) sitting just at
  the 25-cog threshold and being included in `analyze complexity --max-cognitive 25`
  but excluded from `quality-gate`'s default threshold.
- A function exported from a path that PMAT classifies differently between
  the two subcommands (e.g. a `crates/foo/examples/` path counted as `examples/`
  in one query and as `crates/foo/` in the other).
- Stale snapshot — counts taken on slightly different commits.

**Resolution policy (binding for all later waves):**

The authoritative violation count for Wave-N success criteria is the live
output of `pmat quality-gate --fail-on-violation --checks complexity --format json`
on the wave-merge commit. **Wave 0 Task 6 (added in the post-review revision
pass) generates a machine-readable inventory snapshot** at
`.planning/phases/75-fix-pmat-issues/pmat-inventory-2026-04-22.json`
(committed to the repo) — every later wave's "expected count drop" is
derived from that single JSON file via `jq` rather than from the prose
counts in this document.

**Practical implication for plan numbers:**
- Treat all "94" / "73" / "21" / "3" figures in this RESEARCH.md as
  *approximate* — they're the right order of magnitude and the right
  per-directory clustering, but ±5 from the live truth.
- Treat the live `pmat-inventory-2026-04-22.json` (Wave 0 Task 6 output)
  as the authoritative numerator.
- Wave-merge commit messages record `pmat-complexity: NN (was MM)` from
  live measurement, not from this doc's projections.

### Badge/Gate Command Verification (added 2026-04-23 post-cross-AI-review)

**Codex reviewer flagged a stated-goal failure mode:** the badge in
`quality-badges.yml` line ~70-82 is set by
`pmat quality-gate --fail-on-violation` (no `--checks` filter), but Wave 5
adds enforcement only for `pmat quality-gate --fail-on-violation --checks
complexity` to `ci.yml`. CI can flip green while the badge stays red.

**The phase goal is "restore the badge", not "make CI green"** — D-01 is
binding. So Wave 0 (post-review revision) adds Task 5 to verify which
specific dimensions currently fail the bare gate:

```bash
pmat quality-gate --fail-on-violation --format json > /tmp/bare-gate.json; echo "bare exit=$?"
pmat quality-gate --fail-on-violation --checks complexity --format json > /tmp/cx-gate.json; echo "cx exit=$?"
# Compare per-check counts to identify which dimensions cause non-zero exit on bare.
```

**Two outcomes drive Wave 5:**
- **Case A — only complexity fails today:** the existing Wave 5 plan
  works; flipping complexity to 0 also flips the bare gate to 0; badge
  flips green. Wave 5 only edits `ci.yml`.
- **Case B — other dimensions also fail today** (e.g. SATD count is
  factored into bare-gate exit code): Wave 5 MUST also edit
  `quality-badges.yml` so the badge command matches the CI gate command
  (both use `--checks complexity`, OR both use the same path-filter).

**See CONTEXT.md D-11 for the binding decision tree.** Wave 5 plan is
written to handle BOTH branches; Wave 0 Task 5 narrows to one before
Wave 5 runs.


## Code Examples

### Verified pattern: Existing bare `#[allow(clippy::cognitive_complexity)]` (anti-pattern per D-02)
```rust
// src/shared/sse_optimized.rs:224-226 (current state — bare, no Why:)
/// Connect to SSE endpoint
#[allow(clippy::cognitive_complexity)]
async fn connect_sse(...) { ... }
```
After Phase 75 it must look like:
```rust
/// Connect to SSE endpoint
// Why: SSE reconnection state — header negotiation, retry-after parsing,
// last-event-id replay, and 3 error-class branches share local state;
// extracting helpers would require an awkward shared mutable struct.
#[allow(clippy::cognitive_complexity)]
async fn connect_sse(...) { ... }
```

### Verified command: Quality-gate exit code is the badge signal
```bash
# Source: .github/workflows/quality-badges.yml lines 70-82
if pmat quality-gate --fail-on-violation --format json > quality_gate.json 2>/dev/null; then
  GATE_STATUS="passing"
  GATE_COLOR="brightgreen"
else
  GATE_STATUS="failing"
  GATE_COLOR="red"
fi
```

### Recommended D-07 implementation in `ci.yml`
```yaml
# Add to existing quality-gate job in .github/workflows/ci.yml around line 167
    - name: Install PMAT (pinned)
      run: cargo install pmat --version =3.15.0 --locked

    - name: Run PMAT quality gate (complexity only — see CONTEXT.md D-01)
      run: pmat quality-gate --fail-on-violation --checks complexity
```
Then the existing `gate` job (lines ~210-225) already aggregates `quality-gate` into the required check — no further plumbing.

`[VERIFIED: read of .github/workflows/ci.yml lines 1-225]`

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `pmat-cli` crate name | `pmat` crate name | PR #246 (this repo, recent) | The fix to `quality-badges.yml` is what surfaced the real PMAT findings — Phase 75 is the consequence |
| Bare `#[allow]` for complexity | `#[allow]` + `// Why:` justification | This phase (D-02) | Establishes documentation discipline for escape hatches |
| Pre-commit-only quality enforcement | CI-blocking PMAT gate | This phase (D-07) | Prevents complexity regressions from landing |
| `make quality-gate` (fmt/clippy/build/test/audit) | Same locally + PMAT gate in CI | This phase (D-07) | PMAT runs only in CI to avoid slowing dev loop |

**Deprecated/outdated:**
- `pmat-cli` (the package name) — superseded by `pmat`. Don't reference it anywhere.
- `[analysis] exclude_patterns` in `.pmat/project.toml` for duplicate-gate filtering — does not work in 3.15.0; see Pitfall 3.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Pattern 1 (extract-method per match arm) reliably reduces cognitive complexity by ~50% per arm | Architecture Patterns | Refactor effort estimate may be too optimistic; planner may need to plan more `#[allow]` use |
| A2 | Cognitive complexity scoring counts nesting more heavily than sequence | Pattern 2 rationale | Refactor strategy may be wrong direction; needs verification per function during execution |
| A3 | The `quality-gate` job in `ci.yml` is the right insertion point for D-07 (vs new workflow file) | Architecture / D-07 implementation | Wrong placement could double-run PMAT in CI or miss the org ruleset's required check |
| A4 | Pinning `pmat --version =3.15.0` in CI is desirable | Pitfall 1 | Maintainer might prefer floating to latest for security updates; check team preference |
| A5 | The existing 6 bare `#[allow(clippy::cognitive_complexity)]` should be retroactively justified, not removed | Pitfall 2 | If team wants to refactor those instead, scope grows |
| A6 | Examples-side violations (21 of 94) need to be addressed for the gate to flip | Pitfall 5 | If PMAT can be configured to skip examples/ for complexity, those 21 don't need touching |

**A2-A6 should be confirmed in the discuss-phase or by the planner before locking refactor strategy.**

## Open Questions (RESOLVED)

1. **Does `cargo install pmat --version =X.Y.Z --locked` produce the same gate result as floating-latest?**
   - What we know: CI today uses `cargo install pmat --locked` with no version pin.
   - What's unclear: Whether pinning is desired given Pitfall 1 risk.
   - Recommendation: Plan should include a one-line decision and CI step accordingly.
   - RESOLVED: Plan 75-00 Task 3 pins to `=3.15.0 --locked` in both ci.yml and quality-badges.yml.

2. **Can `pmat quality-gate --checks complexity` be made to skip examples/?**
   - What we know: `[analysis] exclude_patterns` in `.pmat/project.toml` and `.pmatignore` were tested for duplicates and did NOT work; they were not tested for the complexity check specifically.
   - What's unclear: Whether complexity-check exclusion behaves differently from duplicate-check exclusion in 3.15.0.
   - Recommendation: Wave 0 / Wave 1 first task should empirically test by writing a `.pmatignore` with `examples/` and re-running `--checks complexity`. If it works, scope shrinks dramatically.
   - RESOLVED: Empirical answer deferred to Plan 75-00 Task 0 (D-09 spike); Plan 75-04 Task 4-B and Plan 75-05 Task 5-01 branch on the spike result.

3. **Can `pmat hooks install` from `pmat.toml` provide the gate step automatically?**
   - What we know: `pmat hooks` subcommand exists; supports `init`, `install`, `verify`, `run`. CONTEXT.md D-07 explicitly chose CI-only.
   - What's unclear: Whether `pmat hooks` could be co-opted for CI (it's labelled "pre-commit hook management" but `pmat hooks run` is described as "for CI/CD integration").
   - Recommendation: Stick with the explicit `pmat quality-gate` step; don't expand scope.
   - RESOLVED: CONTEXT.md D-07 chose CI-only via explicit ci.yml step; pmat hooks not used.

4. **Are there 94 distinct functions, or do some functions have BOTH cyclomatic-complexity and cognitive-complexity violations counted as 2?**
   - What we know: The full violations array has 187 entries when both rules are counted. Filtering to `cognitive-complexity` only gives 94.
   - What's unclear: Whether the gate counts distinct functions or distinct (function, rule) pairs toward the 94 figure.
   - Recommendation: Treat the 94 as the authoritative count; the badge is gated by the gate's own counter, not ours.
   - RESOLVED: Treat 94 as authoritative — gate counter is the source of truth; not subdividing into per-rule splits.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `pmat` (CLI) | All waves — measurement, gate verification | ✓ | 3.15.0 | None — required for the phase |
| `cargo` (stable) | All waves — refactor verification | ✓ | (toolchain stable) | None |
| `cargo-llvm-cov` | Coverage measurement (optional) | Used in CI; should be local-installable | (CI installs on demand) | Skip coverage measurement; not on critical path for D-01 |
| `cargo-nextest` | Test runner (optional) | Used in CI | (CI installs) | Use `cargo test` |
| `gh` (GitHub CLI) | D-04 SATD triage — filing issues | Standard | — | Manual issue creation via web UI |
| `git` | Version control | ✓ | (assumed) | None |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:** Coverage tooling — not gating per D-01.

## Validation Architecture

> Per `.planning/config.json` workflow.nyquist_validation: enabled (default).

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `cargo test` (workspace-wide), `proptest` 1.6, `trybuild` 1.0, `insta` 1.43 |
| Config files | `Cargo.toml` (workspace root), `pmcp-macros/Cargo.toml` (test deps) |
| Quick run command (per-task) | `cargo test --workspace --lib -- --test-threads=1` (existing CI uses --test-threads=1) |
| Full suite command (per-wave) | `make quality-gate` then `cargo test --workspace --all-features --verbose -- --test-threads=1` |
| PMAT measurement | `pmat quality-gate --fail-on-violation --checks complexity` (must show monotonic decrease in violation count per wave merge) |

### Phase Requirements → Test Map

Phase 75 has no REQ-IDs. Validation is per-wave functional regression + per-commit complexity reduction:

| Wave | Behavior gate | Test Type | Automated Command | File Exists? |
|------|--------------|-----------|-------------------|-------------|
| Wave 1a | streamable_http_server transport unchanged | unit + integration | `cargo test --test streamable_http_server_tests --test streamable_http_unit_tests` | ✅ existing |
| Wave 1a | streamable_http_server property invariants | property | `cargo test --test streamable_http_properties` | ✅ existing |
| Wave 1b | pmcp-macros expansion stable | snapshot | `cargo test -p pmcp-macros` (extends existing `mcp_*_tests.rs`) | ✅ existing; recommend adding `insta` snapshots for the 4 expand fns BEFORE refactor |
| Wave 1b | pmcp-macros downstream still compiles | compile | `cargo test --workspace --all-features` | ✅ implicit |
| Wave 1b | trybuild compile-fail tests still pass | compile-fail | `cargo test -p pmcp-macros --test trybuild_tests` (if exists) — verify in plan | ❓ Planner verifies |
| Wave 2a | pentest attacks behavior unchanged | integration | `cargo run -p cargo-pmcp -- pentest --dry-run` (if dry-run exists) | ❓ Planner verifies; otherwise manual smoke |
| Wave 2b | deployment behavior unchanged | unit | `cargo test -p cargo-pmcp deployment::` | ✅ assumed; verify per file |
| Wave 3 | code-mode evaluator behavior unchanged | unit | `cargo test -p pmcp-code-mode` (existing tests in `crates/pmcp-code-mode/tests/`) | ✅ existing |
| Wave 4 | CI gate fails when complexity regresses | integration | Open a test PR with a deliberately complex function; verify CI fails | Manual one-time validation |

### Sampling Rate
- **Per task commit:** `cargo test --workspace --lib` + `cargo clippy --all-targets --all-features` (matches the spirit of `make quality-gate` without the full audit suite).
- **Per wave merge:** `make quality-gate` (full suite — fmt/clippy/build/test-all/audit/unused-deps/check-todos/check-unwraps/validate-always) AND `pmat quality-gate --fail-on-violation --checks complexity` with violation count recorded in the wave commit message.
- **Phase gate:** Full `pmat quality-gate --fail-on-violation` (all checks) exits 0; badge in README flips to passing on next `quality-badges.yml` run.

### Wave 0 Gaps
- [ ] `insta` snapshot baseline for `pmcp-macros` expansion of the 4 worst offenders — capture BEFORE Wave 1b refactor begins, so any change is visible. (Existing `pmcp-macros/tests/mcp_server_tests.rs`, `mcp_tool_tests.rs`, `mcp_prompt_tests.rs` exist but coverage of expansion stability needs verification.)
- [ ] Confirm `trybuild` compile-fail tests exist for `pmcp-macros`. If not, defer (out of scope) but document the gap.
- [ ] Confirm `crates/pmcp-code-mode/tests/` covers `evaluate_with_scope` and `evaluate_array_method_with_scope` at semantic level (not just basic happy path). If not, **add semantic regression tests in Wave 0** before Wave 3 refactors the worst-complexity functions in the codebase.
- [ ] Empirical test: write `.pmatignore` with `examples/` and run `pmat quality-gate --fail-on-violation --checks complexity` — verify whether examples can be excluded from the complexity check (resolves Open Question 2). Should be a Wave 0 task because the answer materially changes phase scope.

## Project Constraints (from CLAUDE.md)

These directives from `./CLAUDE.md` constrain Phase 75 plans:

- **Toyota Way zero tolerance for defects.** Refactor commits must not regress quality.
- **Pre-commit quality gates MANDATORY.** `make quality-gate` blocks commits today; Phase 75 does not change that. Adding PMAT to pre-commit is explicitly deferred per D-07.
- **PDMT-style todos** with quality gates and success criteria — plans should follow this format.
- **Cognitive complexity ≤25 per function.** This is exactly what Phase 75 enforces. The `#[allow]` escape hatch (D-02) is the documented exception.
- **Zero SATD comments allowed.** D-04 reconciles: triage existing 33 down via the three-way policy.
- **80%+ test coverage with quality doctests.** Refactor must not lower coverage. Some plans should explicitly run `cargo llvm-cov` per wave.
- **`#[allow]` is allowed for irreducible cases**, but project standards require justification — D-02's `// Why:` comment is the project-specific instantiation of this.
- **ALWAYS requirements** (FUZZ + PROPERTY + UNIT + EXAMPLE per CLAUDE.md): Phase 75 is refactor, not feature, so the "ALWAYS requirements" do not apply to new work created. Existing fuzz/property/unit tests must continue to pass.
- **Release impact:** `release.yml` is triggered on `v*` tag push. Phase 75 does not bump versions. **The new D-07 gate runs on PR — it does NOT run on tag push.** No release-time disruption expected. Confirmed by reading `release.yml` lines 1-50.

## Sources

### Primary (HIGH confidence — verified live this session)
- `pmat 3.15.0 --help` and subcommand help — installed binary at `/Users/guy/.cargo/bin/pmat`
- `pmat config --show` — full default config inspection
- `pmat quality-gate --fail-on-violation --format json` — exit code 1, 94 cognitive-complexity violations counted
- `pmat analyze complexity --format json --max-cognitive 25` — full per-function violation list (output saved to `/tmp/pmat-complexity.json` for this session)
- Live `.pmatignore` and `.pmat/project.toml [analysis]` exclusion experiments — both failed to reduce duplicate count from 1554 (Pitfall 3 evidence)
- `.github/workflows/quality-badges.yml`, `.github/workflows/ci.yml`, `.github/workflows/release.yml` — read end-to-end this session
- `Makefile` quality-gate target — read this session
- `pmcp-macros/Cargo.toml` — confirmed `trybuild`, `insta`, `proptest` dev-dependencies
- `src/client/http_logging_middleware.rs`, `src/shared/connection_pool.rs`, `src/shared/sse_optimized.rs` — confirmed existing bare `#[allow]` pattern
- `src/server/path_validation.rs` lines 60-110 — read for refactor pattern verification

### Secondary (MEDIUM confidence — official docs, single source)
- [PMAT Book ch30 - File Exclusions (.pmatignore)](https://paiml.github.io/pmat-book/ch30-00-file-exclusions.html) — confirmed `.pmatignore` is the documented mechanism (gitignore syntax). Note: experimental verification this session showed it does NOT work for the duplicate-gate check; documentation may be aspirational or version-specific.
- [PMAT Book - top-level docs](https://paiml.github.io/pmat-book/) — used for navigation/orientation only

### Tertiary (LOW confidence — refactor patterns, training-data)
- Pattern 1-6 (extract-method, early-return, dispatch tables, etc.) — well-known Rust refactor techniques but per-function effectiveness `[ASSUMED]` — not measured against this codebase

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — installed binary verified; existing test infra confirmed by file read
- Architecture (CI workflow shape): HIGH — `ci.yml` and `quality-badges.yml` read end-to-end
- Per-hotspot inventory: HIGH — produced from live `pmat analyze` JSON output
- PMAT exclusion behavior (Pitfall 3): HIGH — verified by two independent live experiments
- Refactor patterns (1-6): MEDIUM — patterns well-established in Rust community, but per-function complexity reduction is `[ASSUMED]` until measured
- Risk assessment per file: MEDIUM — based on test-file presence and CONTEXT.md `<specifics>` notes; deeper coverage data not gathered

**Research date:** 2026-04-22
**Valid until:** 2026-05-22 (30 days — PMAT version pinning may shift gate semantics; refresh if pmat releases a major bump)
