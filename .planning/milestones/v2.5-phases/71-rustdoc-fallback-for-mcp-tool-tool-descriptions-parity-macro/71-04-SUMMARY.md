---
phase: 71
plan: 04
subsystem: release-mechanics
tags: [release, version-bump, changelog, requirements, quality-gate, ripple-audit]
requires:
  - 71-01-SUMMARY.md (pmcp-macros-support 0.1.0 landed)
  - 71-02-SUMMARY.md (shared resolve_tool_args + wired parse sites)
  - 71-03-SUMMARY.md (README doctest + trybuild snapshots + fuzz target)
provides:
  - pmcp 2.4.0 (MINOR) release marker
  - pmcp-macros 0.6.0 release marker
  - cargo-pmcp 0.7.1 concurrent downstream patch bump
  - mcp-tester 0.5.1 concurrent downstream patch bump
  - CHANGELOG 2.4.0 entry with PARITY-MACRO-01 traceability
  - REQUIREMENTS.md row 145 Phase 71 Complete
  - Quality-gate green sign-off
affects:
  - Cargo.toml
  - pmcp-macros/Cargo.toml
  - cargo-pmcp/Cargo.toml
  - crates/mcp-tester/Cargo.toml
  - CHANGELOG.md
  - .planning/REQUIREMENTS.md
  - pmcp-macros/src/mcp_common.rs (lint fixes)
  - tests/handler_extensions_properties.rs (lint fix)
tech-stack:
  added: []
  patterns: [semver-minor-for-additive-feature, caret-pin-ripple-audit]
key-files:
  created:
    - .planning/phases/71-.../71-04-SUMMARY.md
  modified:
    - Cargo.toml (pmcp 2.3.0 → 2.4.0 + 2x pmcp-macros pins 0.5.0 → 0.6.0)
    - pmcp-macros/Cargo.toml (0.5.0 → 0.6.0)
    - cargo-pmcp/Cargo.toml (0.7.0 → 0.7.1, concurrent downstream bump)
    - crates/mcp-tester/Cargo.toml (0.5.0 → 0.5.1, concurrent downstream bump)
    - CHANGELOG.md (new 2.4.0 section)
    - .planning/REQUIREMENTS.md (line 56 ticked, row 145 Phase 71/Complete, footer)
    - pmcp-macros/src/mcp_common.rs (4x pub(crate) → pub, 1x doc_markdown fix)
    - tests/handler_extensions_properties.rs (1x doc_markdown fix)
decisions:
  - "MEDIUM-4: pmcp 2.3.0 → 2.4.0 MINOR (not patch) — #[mcp_tool] now accepts rustdoc-only functions, additive feature surface per semver convention"
  - "HIGH-2: workspace ripple audit executed — no non-caret pmcp pins found, all existing pins accept 2.4.0"
  - "W2 Option A (CLAUDE.md discipline): concurrent patch bumps on cargo-pmcp 0.7.0→0.7.1 + mcp-tester 0.5.0→0.5.1 even though caret pins accept 2.4.0"
  - "Rule 3 deviation: fixed 5 pre-existing clippy pedantic+nursery lints (4 redundant_pub_crate in mcp_common.rs + 2 doc_markdown) blocking quality-gate required by this plan"
metrics:
  duration_minutes: 20
  completed: 2026-04-18
---

# Phase 71 Plan 04: Release Mechanics Summary

Version-bump + changelog + requirements traceability + `make quality-gate` sign-off for Phase 71 (rustdoc fallback for `#[mcp_tool]`). Honors CLAUDE.md §"Version Bump Rules" concurrent-downstream-bump discipline.

## One-liner

pmcp 2.4.0 (MINOR, additive) + pmcp-macros 0.6.0 + pmcp-macros-support 0.1.0 released with PARITY-MACRO-01 closure, concurrent patch bumps on cargo-pmcp 0.7.1 + mcp-tester 0.5.1, and `make quality-gate` green.

## Commits (3)

| SHA        | Subject                                                                                       |
|------------|-----------------------------------------------------------------------------------------------|
| `dd9a1ad1` | `chore(71-04): bump pmcp 2.3.0→2.4.0 + pmcp-macros 0.5.0→0.6.0 + downstream ripple`           |
| `71c731f5` | `docs(71-04): CHANGELOG 2.4.0 entry + REQUIREMENTS PARITY-MACRO-01 closure`                   |
| `bf0370de` | `fix(71-04): resolve clippy pedantic+nursery lints to unblock make quality-gate`              |

## Workspace `pmcp`-dependency Ripple Audit (HIGH-2)

Explicit grep executed per Codex review HIGH-2 directive:

```bash
$ grep -rn '^pmcp = ' Cargo.toml cargo-pmcp/Cargo.toml crates/*/Cargo.toml fuzz/Cargo.toml
cargo-pmcp/Cargo.toml:38:pmcp = { version = "2.2.0", path = "..", features = ["streamable-http", "oauth"] }
crates/mcp-tester/Cargo.toml:21:pmcp = { version = "2.2.0", path = "../../", features = ["streamable-http", "oauth"] }
crates/pmcp-code-mode-derive/Cargo.toml:26:pmcp = { version = ">=2.2.0", path = "../../" }
crates/pmcp-code-mode/Cargo.toml:17:pmcp = { version = ">=2.2.0", path = "../../" }
crates/pmcp-server/Cargo.toml:30:pmcp = { version = "2.2.0", path = "../../", features = ["streamable-http"] }
crates/pmcp-tasks/Cargo.toml:10:pmcp = { version = "2.2.0", path = "../..", default-features = false }
crates/pmcp-tasks/Cargo.toml:32:pmcp = { version = "2.2.0", path = "../..", features = ["full"] }

$ grep -rn 'pmcp = "= ' Cargo.toml cargo-pmcp/ crates/ fuzz/ 2>/dev/null
# (empty — no exact pins)

$ grep -rn 'pmcp = "~' Cargo.toml cargo-pmcp/ crates/ fuzz/ 2>/dev/null
# (empty — no tilde pins)
```

Each pin resolution:
- `"2.2.0"` is caret-shape by Cargo default — accepts 2.4.0. **No forced bump.**
- `">=2.2.0"` is lower-bound-only — accepts 2.4.0. **No forced bump.**
- 7 pins total across the workspace; 0 non-caret pins.

No downstream crate REQUIRED a bump from 2.4.0 semantics. The concurrent patch bumps on cargo-pmcp + mcp-tester below are CLAUDE.md-discipline-driven (W2 Option A), not resolution-mechanics-driven.

## Version Bumps Applied (5 strings)

| Crate                  | Before    | After     | Driver                                                           |
|------------------------|-----------|-----------|------------------------------------------------------------------|
| `pmcp`                 | `2.3.0`   | `2.4.0`   | MEDIUM-4: additive `#[mcp_tool]` source form → MINOR             |
| `pmcp-macros`          | `0.5.0`   | `0.6.0`   | Rustdoc fallback feature lands in this crate (MINOR)             |
| `pmcp-macros-support`  | `0.1.0`   | `0.1.0`   | New crate from Plan 01 — no bump                                 |
| Root `Cargo.toml:53`   | `"0.5.0"` | `"0.6.0"` | Optional prod dep pin (matches pmcp-macros bump)                 |
| Root `Cargo.toml:147`  | `"0.5.0"` | `"0.6.0"` | Dev-dep pin for s23 macro example                                |
| `cargo-pmcp`           | `0.7.0`   | `0.7.1`   | Concurrent downstream patch (CLAUDE.md §"Version Bump Rules")    |
| `mcp-tester`           | `0.5.0`   | `0.5.1`   | Concurrent downstream patch (same discipline)                    |

**Deviation from plan (Rule 1/3):** Plan text said cargo-pmcp `0.6.0 → 0.6.1`, but the tree had cargo-pmcp at `0.7.0` when this plan started. Applied the plan's intent (patch bump for concurrent downstream discipline) from the actual starting version: `0.7.0 → 0.7.1`.

## CHANGELOG.md

New `## [2.4.0] - 2026-04-17` section above `## [2.3.0]` with:
- **Added:** pmcp-macros 0.6.0 rustdoc fallback (PARITY-MACRO-01), pmcp-macros-support 0.1.0 new crate, README migration section.
- **Changed:** updated error message wording, pmcp MINOR bump rationale, pmcp-macros bump, cargo-pmcp + mcp-tester concurrent patch bumps.
- **Internal:** pmcp-macros-support tests + 4 proptest invariants, 2 new trybuild snapshots, rustdoc_normalize fuzz target, resolve_tool_args shared resolver.

Acceptance grep results:
- `grep -c '## \[2.4.0\]' CHANGELOG.md` = 1.
- `grep -c 'PARITY-MACRO-01' CHANGELOG.md` = 1.
- `grep -c 'rustdoc fallback' CHANGELOG.md` = 1.
- `grep -c 'pmcp-macros-support' CHANGELOG.md` = 3.
- `grep -c 'cargo-pmcp.*0\.7\.1' CHANGELOG.md` = 1.
- `grep -c 'mcp-tester.*0\.5\.1' CHANGELOG.md` = 2.

## REQUIREMENTS.md Updates

- **Line 56 checkbox** (PARITY-MACRO-01): `- [ ]` → `- [x]`.
- **Row 145 traceability table**: `| PARITY-MACRO-01 | TBD | Pending |` → `| PARITY-MACRO-01 | Phase 71 | Complete |`.
- **Footer** appended: `*Last updated: 2026-04-17 — PARITY-MACRO-01 closed by Phase 71 (pmcp 2.4.0 / pmcp-macros 0.6.0 / pmcp-macros-support 0.1.0 — rustdoc fallback)*`.

## Quality Gate Sign-off

```
$ make quality-gate
...
[0;32m✓ Code formatting OK[0m
[0;32m✓ No lint issues[0m
[0;32m✓ Property tests passed[0m
[0;32m✓ Unit tests passed[0m
[0;32m        ✅ ALL TOYOTA WAY QUALITY CHECKS PASSED        [0m
EXIT: 0
```

**Deviation — auto-fixed (Rule 3):** `make quality-gate` initially failed with 5 clippy pedantic+nursery lints that were missed by prior Phase 71 plans (which ran `cargo check` but not the full pedantic gate):

1. `pmcp-macros/src/mcp_common.rs:332` — `pub(crate) const MCP_TOOL_MISSING_DESCRIPTION_ERROR` → `pub const` (redundant_pub_crate: module is already private).
2. `pmcp-macros/src/mcp_common.rs:343` — `pub(crate) fn build_description_meta` → `pub fn` (same lint).
3. `pmcp-macros/src/mcp_common.rs:349` — doc_markdown: `NameValue` → `` `NameValue` ``.
4. `pmcp-macros/src/mcp_common.rs:352` — `pub(crate) fn has_description_meta` → `pub fn`.
5. `pmcp-macros/src/mcp_common.rs:382` — `pub(crate) fn resolve_tool_args` → `pub fn`.
6. `tests/handler_extensions_properties.rs:4` — doc_markdown: `remove::<T>()` → `` `remove::<T>()` `` (pre-existing; fixed because it blocked this plan's quality-gate).

All items are pub-but-in-private-module so external visibility is unchanged; the module itself (`mcp_common`) is crate-internal in pmcp-macros.

## Additional CI Evidence

| Gate                                                                 | Exit |
|----------------------------------------------------------------------|------|
| `cargo check --workspace --features full`                            | 0    |
| `cargo check --workspace --examples --features full` (25 sites)      | 0    |
| `cargo test --doc -p pmcp-macros` (README doctest + 10 passing)      | 0    |
| `cargo test -p pmcp-macros-support` (20 passed across 3 suites)      | 0    |
| `cargo test -p pmcp-macros` (87 passed, 4 trybuild UI snapshots ok)  | 0    |
| `cd fuzz && cargo build --bin rustdoc_normalize`                     | 0    |
| `make quality-gate`                                                  | 0    |

## Updated Publish Order (2.4.0 Release Cycle)

Per CLAUDE.md §"Release & Publish Workflow" — new publish order to include `pmcp-macros-support` as a leaf dependency of `pmcp-macros`:

1. `pmcp-widget-utils` (leaf)
2. **`pmcp-macros-support` (new, leaf — no internal deps beyond syn/proc-macro2)**
3. `pmcp-macros` (depends on pmcp-macros-support transitively via property/fuzz tests)
4. `pmcp` (depends on pmcp-macros via optional `macros` feature)
5. `mcp-tester`
6. `mcp-preview`
7. `cargo-pmcp`

**Follow-up:** Updating CLAUDE.md §"Release & Publish Workflow" itself is out of scope for Phase 71. The next release-plan PR should land the CLAUDE.md text update.

## Diff Stat

```
 .planning/REQUIREMENTS.md              |  5 +++--
 CHANGELOG.md                           | 20 ++++++++++++++++++++
 Cargo.toml                             |  6 +++---
 cargo-pmcp/Cargo.toml                  |  2 +-
 crates/mcp-tester/Cargo.toml           |  2 +-
 pmcp-macros/Cargo.toml                 |  2 +-
 pmcp-macros/src/mcp_common.rs          | 14 +++++++-------
 tests/handler_extensions_properties.rs |  2 +-
 8 files changed, 37 insertions(+), 16 deletions(-)
```

## Phase 71 Must-Haves Sign-off (10/10)

| # | Must-have                                                                             | Evidence                                                                 |
|---|---------------------------------------------------------------------------------------|--------------------------------------------------------------------------|
| 1 | Rustdoc-only fn compiles with normalized description                                  | Plan 02 `test_rustdoc_only_description`; Plan 03 README doctest          |
| 2 | Attribute wins byte-for-byte on 3 s23 sites                                           | Plan 02 + `cargo check --workspace --examples` green                     |
| 3 | Neither-present compile-fail locked by trybuild snapshots                             | Plan 03 + Task 3 `cargo test -- compile_fail_tests` green                |
| 4 | `description = ""` semantic locked                                                    | Plan 02 unit tests + Plan 03 Limitations                                 |
| 5 | Unsupported rustdoc forms documented                                                  | Plan 01 + Plan 03 Limitations                                            |
| 6 | All 25 call sites compile unchanged                                                   | `cargo check --workspace --examples --features full` exit 0              |
| 7 | `make quality-gate` passes                                                            | This plan's Task 3 — EXIT: 0                                             |
| 8 | ALWAYS requirements met (fuzz, property, unit, example)                               | Plan 03 fuzz target + Plan 01 property + unit + Plan 03 doctest          |
| 9 | Both parse sites byte-symmetric via shared resolver                                   | Plan 02 + symmetric integration tests                                    |
| 10| pmcp-macros 0.6.0 + pmcp 2.4.0 + pmcp-macros-support 0.1.0                            | This plan Task 1                                                         |
| 11| Workspace pmcp ripple audit (HIGH-2)                                                  | This SUMMARY §"Workspace `pmcp`-dependency Ripple Audit"                 |
| 12| PARITY-MACRO-01 ticked in REQUIREMENTS.md                                             | This plan Task 2                                                         |

## Deviations from Plan

### Auto-fixed (Rule 3 — blocking issue)

**1. [Rule 3 - Blocking] Fixed 5 clippy pedantic+nursery lints to unblock `make quality-gate`**
- **Found during:** Task 3 (running `make quality-gate`)
- **Issue:** Prior Phase 71 plans (01–03) added `pub(crate)` items to `mcp_common.rs` inside an already-private module (redundant_pub_crate lint) and used un-backticked type names in doc comments (doc_markdown lint). Also, an unrelated pre-existing `tests/handler_extensions_properties.rs` had the same doc_markdown issue. Together these 6 lints broke the quality-gate that CLAUDE.md mandates for any commit.
- **Fix:** 4x `pub(crate)` → `pub` in mcp_common.rs; 2x doc_markdown backtick fixes.
- **Files:** `pmcp-macros/src/mcp_common.rs`, `tests/handler_extensions_properties.rs`.
- **Commit:** `bf0370de`.

**2. [Rule 1 - Divergence] cargo-pmcp starting version was 0.7.0, not 0.6.0**
- **Found during:** Task 1
- **Issue:** Plan text specified cargo-pmcp bump `0.6.0 → 0.6.1`, but the actual tree version was `0.7.0` (likely bumped by an intervening PR).
- **Fix:** Applied the plan's INTENT (concurrent downstream patch bump per CLAUDE.md §"Version Bump Rules") from the actual starting version: `0.7.0 → 0.7.1`. Updated CHANGELOG accordingly.
- **Files:** `cargo-pmcp/Cargo.toml`, `CHANGELOG.md`.
- **Commit:** `dd9a1ad1` (cargo-pmcp bump) + `71c731f5` (CHANGELOG).

### Plan-text adjustments

- Plan Task 3 Step 1 command `cargo test --doc -p pmcp-macros --features full` failed because `pmcp-macros` has no `full` feature (only `debug`). Ran `cargo test --doc -p pmcp-macros` instead (10 passed). Plan text typo; functional equivalent executed.

## Self-Check: PASSED

- Commit `dd9a1ad1`: FOUND in `git log --oneline --all`.
- Commit `71c731f5`: FOUND.
- Commit `bf0370de`: FOUND.
- File `.planning/phases/71-.../71-04-SUMMARY.md`: (this file, will be committed next).
- `grep -c '^version = "2.4.0"' Cargo.toml` = 1 ✓
- `grep -c '^version = "0.6.0"' pmcp-macros/Cargo.toml` = 1 ✓
- `grep -c '^version = "0.7.1"' cargo-pmcp/Cargo.toml` = 1 ✓
- `grep -c '^version = "0.5.1"' crates/mcp-tester/Cargo.toml` = 1 ✓
- `grep -c 'PARITY-MACRO-01 | Phase 71 | Complete' .planning/REQUIREMENTS.md` = 1 ✓
- `make quality-gate` exit 0 ✓
