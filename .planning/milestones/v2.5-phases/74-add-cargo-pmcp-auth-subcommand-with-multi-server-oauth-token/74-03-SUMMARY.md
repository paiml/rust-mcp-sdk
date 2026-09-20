---
phase: 74
plan: 03
subsystem: release-coordination
tags: [release, semver, changelog, quality-gate, pmcp, cargo-pmcp, mcp-tester]
requires:
  - phase: 74-01
    provides: "SDK DCR (RFC 7591) + AuthorizationResult + OAuthConfig new shape landed on main"
  - phase: 74-02
    provides: "cargo-pmcp auth command group + TokenCacheV1 landed on main"
provides:
  - "pmcp 2.5.0 version literal on main (Cargo.toml)"
  - "cargo-pmcp 0.9.0 version literal on main (cargo-pmcp/Cargo.toml)"
  - "mcp-tester 0.5.2 version literal on main (crates/mcp-tester/Cargo.toml)"
  - "8 workspace-internal pmcp dep pins updated to 2.5.0 across 7 Cargo.toml files"
  - "CHANGELOG.md v2.5.0 entry dated 2026-04-21 (no <unreleased> placeholder remains)"
  - "make quality-gate exits 0 on HEAD (CI-matching gate: fmt + clippy pedantic+nursery + build + test + audit + examples)"
affects:
  - "Phase 74 is release-ready; operator can drive the git tag + cargo publish steps per CLAUDE.md Release & Publish Workflow"
tech-stack:
  added: []
  patterns:
    - "Version-bump trio: pmcp (SDK) + cargo-pmcp (CLI consumer) + mcp-tester (SDK consumer) released together"
    - "Workspace pin audit via closing grep gate: grep -rnE 'pmcp = \\{ version = \"2\\.[234]\\.0\"' returns 0 post-bump"
    - "Source-state-only release plan: tagging/publishing stays operator-driven; this plan ships the committable state"
key-files:
  created:
    - .planning/phases/74-add-cargo-pmcp-auth-subcommand-with-multi-server-oauth-token/74-03-SUMMARY.md
  modified:
    - Cargo.toml
    - cargo-pmcp/Cargo.toml
    - crates/mcp-tester/Cargo.toml
    - crates/pmcp-server/Cargo.toml
    - crates/pmcp-server/pmcp-server-lambda/Cargo.toml
    - crates/pmcp-tasks/Cargo.toml
    - examples/25-oauth-basic/Cargo.toml
    - examples/test-basic/Cargo.toml
    - CHANGELOG.md
key-decisions:
  - "pmcp 2.4.0 -> 2.5.0 (minor): new additive DCR public API + OAuthConfig breaking-within-v2.x-window shape change (D-22)"
  - "cargo-pmcp 0.8.1 -> 0.9.0 (minor): new `auth` command group is a new public CLI surface (D-22)"
  - "mcp-tester 0.5.1 -> 0.5.2 (patch): Blocker #1 follow-on — source-level OAuthConfig construction updated under Plan 01; patch-bump is the right semver since there is no new public API, only pmcp-dep consumption"
  - "pmcp-macros, pmcp-widget-utils, mcp-preview NOT bumped — no behavior change this phase (D-22 / D-23 exclusion)"
  - "pmcp-code-mode, pmcp-code-mode-derive, pmcp-macros pin lines LEFT AS-IS — they use `>=` range pins which already encompass 2.5.0"
  - "Tagging + publish NOT performed here (CLAUDE.md Release & Publish Workflow §Release Steps is operator-driven; this plan produces the committable source state)"
requirements-completed: [SDK-DCR-01, CLI-AUTH-01]
duration: "9 min"
completed: 2026-04-21
---

# Phase 74 Plan 03: Release Coordination (pmcp 2.5.0 + cargo-pmcp 0.9.0 + mcp-tester 0.5.2) Summary

Finalized Phase 74 as one coordinated source-state release: bumped three crate versions, updated eight workspace-internal `pmcp` dep pins to 2.5.0, dated the CHANGELOG entry, and confirmed `make quality-gate` is green on HEAD. Tagging and `cargo publish` stay with the operator.

## Performance

- **Duration:** ~9 min (task commit → quality-gate → SUMMARY)
- **Tasks:** 2 of 2 complete
- **Commits:** 1 task commit + 1 metadata commit (this SUMMARY)
- **Files modified:** 9 (8 Cargo.toml + CHANGELOG.md)
- **Rust toolchain used locally:** `rustc 1.95.0 (59807616e 2026-04-14)` — stable per `rust-toolchain.toml`; matches CI's `dtolnay/rust-toolchain@stable`.
- **quality-gate wall-clock:** 461 s (~7 min 41 s) — includes fmt + clippy pedantic+nursery + workspace build + doc-tests + full example matrix + ALWAYS requirements (fuzz/property/unit/example).

## Exact Version-Literal Diffs Applied (Task 3.1)

| File | Line | Before | After |
| --- | --- | --- | --- |
| `Cargo.toml` | 3 | `version = "2.4.0"` | `version = "2.5.0"` |
| `cargo-pmcp/Cargo.toml` | 3 | `version = "0.8.1"` | `version = "0.9.0"` |
| `cargo-pmcp/Cargo.toml` | 38 | `pmcp = { version = "2.2.0", path = "..", features = ["streamable-http", "oauth"] }` | `pmcp = { version = "2.5.0", ... }` |
| `crates/mcp-tester/Cargo.toml` | 3 | `version = "0.5.1"` | `version = "0.5.2"` |
| `crates/mcp-tester/Cargo.toml` | 21 | `pmcp = { version = "2.2.0", path = "../../", features = [...] }` | `pmcp = { version = "2.5.0", ... }` |
| `crates/pmcp-server/Cargo.toml` | 30 | `pmcp = { version = "2.2.0", path = "../../", features = ["streamable-http"] }` | `pmcp = { version = "2.5.0", ... }` |
| `crates/pmcp-server/pmcp-server-lambda/Cargo.toml` | 17 | `pmcp = { version = "2.2.0", path = "../../..", features = [...] }` | `pmcp = { version = "2.5.0", ... }` |
| `crates/pmcp-tasks/Cargo.toml` | 10 | `pmcp = { version = "2.2.0", path = "../..", default-features = false }` | `pmcp = { version = "2.5.0", ... }` |
| `crates/pmcp-tasks/Cargo.toml` | 32 | `pmcp = { version = "2.2.0", path = "../..", features = ["full"] }` | `pmcp = { version = "2.5.0", ... }` |
| `examples/25-oauth-basic/Cargo.toml` | 24 | `pmcp = { version = "2.2.0", path = "../../", features = ["streamable-http"] }` | `pmcp = { version = "2.5.0", ... }` |
| `examples/test-basic/Cargo.toml` | 13 | `pmcp = { version = "2.2.0", path = "../../", features = ["http"] }` | `pmcp = { version = "2.5.0", ... }` |
| `CHANGELOG.md` | 8 | `## [2.5.0] - <unreleased>` | `## [2.5.0] - 2026-04-21` |

**Pin audit totals:** 8 literal `pmcp = { version = "2.2.0", ... }` pins across 7 Cargo.toml files → all 8 updated to `"2.5.0"`. Matches the review LOW-9 corrected count.

**Files EXCLUDED from the pin bump (range pins, correct as-is):**

- `pmcp-macros/Cargo.toml:28` — `pmcp = { version = ">=1.20.0", ... }`
- `crates/pmcp-code-mode/Cargo.toml` — `pmcp = { version = ">=2.2.0", ... }`
- `crates/pmcp-code-mode-derive/Cargo.toml` — `pmcp = { version = ">=2.2.0", ... }`

## Acceptance Criteria — Task 3.1 (all PASS)

| Criterion | Command | Result |
| --- | --- | --- |
| G27 pmcp 2.5.0 | `grep -cE '^version = "2\.5\.0"' Cargo.toml` | `1` ✅ |
| G27 cargo-pmcp 0.9.0 | `grep -cE '^version = "0\.9\.0"' cargo-pmcp/Cargo.toml` | `1` ✅ |
| G27 cargo-pmcp pmcp-pin = 2.5.0 | `grep -cE 'pmcp = \{ version = "2\.5\.0"' cargo-pmcp/Cargo.toml` | `1` ✅ |
| CHANGELOG date finalized | `grep -cE '^## \[2\.5\.0\] - [0-9]{4}-[0-9]{2}-[0-9]{2}' CHANGELOG.md` | `1` ✅ |
| No `<unreleased>` left | `grep -c '<unreleased>' CHANGELOG.md` | `0` ✅ |
| Blocker #3 closing grep gate | `grep -rnE 'pmcp = \{ version = "2\.[234]\.0"' --include=Cargo.toml .` | `0` ✅ (no stale pins) |
| Positive 2.5.0 pin count (≥ 8) | same pattern with `2\.5\.0` across 7 files | `8` hits (7 files; pmcp-tasks contributes 2) ✅ |
| Blocker #1 follow-on | `grep -cE '^version = "0\.5\.2"' crates/mcp-tester/Cargo.toml` | `1` ✅ |
| `cargo check --workspace --features full` | | exit 0 ✅ (31.4s) |

## Acceptance Criteria — Task 3.2 (all PASS)

| Criterion | Result |
| --- | --- |
| G28 `make quality-gate` exits 0 | ✅ (461 s) — "ALL TOYOTA WAY QUALITY CHECKS PASSED" + "ALWAYS Requirements Validated" |
| `cargo check --workspace --features full` exits 0 | ✅ |
| `cargo build --example c08_oauth_dcr --features oauth` | ✅ 0 crates compiled (cached) |
| `cargo test -p pmcp --lib oauth_config_tests` | ✅ 3 passed, 862 filtered out |
| `cargo test -p pmcp --test oauth_dcr_integration --features oauth` | ✅ 5 passed |
| `cargo test -p cargo-pmcp --test auth_integration` | ✅ 7 passed |
| No new `#[allow(clippy::...)]` introduced this plan | ✅ 0 new attributes in the Plan 03 diff (pure Cargo.toml + CHANGELOG edits) |

## Quality Gate Drift

**None.** `make quality-gate` exited 0 on the first run immediately after the Task 3.1 commit. No format drift, no clippy lints, no test failures, no audit findings. Plan 01 and Plan 02 both landed with `make quality-gate` green (per their SUMMARY self-checks), so the version bumps did not surface any latent issues.

**No tests gated with `#[ignore]`** during quality-gate. The Plan 02 deferral-preamble in this plan's action ("If timeout on sub-process CLI tests, mark `#[ignore]`") was not needed — those tests ran cleanly inside the 461-s window.

## CHANGELOG Excerpt (post-edit)

```markdown
## [2.5.0] - 2026-04-21

### Added

- **pmcp 2.5.0 — Dynamic Client Registration (RFC 7591) support in `OAuthHelper`** (Phase 74).
  OAuthConfig gains client_name: Option<String> and dcr_enabled: bool (default: true).
  ...
- **`OAuthHelper::authorize_with_details()` + `AuthorizationResult` struct** (Phase 74, Blocker #6).
- **cargo-pmcp 0.9.0 — `cargo pmcp auth` command group** (Phase 74, Plan 02).
  Five subcommands (login, logout, status, token, refresh) manage per-server
  OAuth tokens in a new ~/.pmcp/oauth-cache.json (schema_version: 1).
  ...

### Changed

- **BREAKING (minor-within-v2.x window):** OAuthConfig::client_id type changed String -> Option<String>
  to enable DCR auto-trigger when client_id.is_none(). ...
- **cargo-pmcp pentest**: migrated from local --api-key flag to shared AuthFlags.
  ...
```

The entry references **both SDK-DCR-01 (DCR in OAuthHelper + AuthorizationResult)** and **CLI-AUTH-01 (cargo pmcp auth command group + pentest migration)** — no further CHANGELOG additions needed.

## Task Commits

1. **Task 3.1 (chore): bump pmcp 2.4.0 → 2.5.0, cargo-pmcp 0.9.0, mcp-tester 0.5.2 + workspace pin audit** — `87a1d100`
2. **Task 3.2 (quality-gate diagnostic)** — no file changes produced; quality-gate exited 0 on first run. Metadata + this SUMMARY committed via the plan metadata commit below.

## Decisions Made

See `key-decisions` in frontmatter. Highlights:

- **pmcp 2.4.0 → 2.5.0** is a minor bump despite the `OAuthConfig::client_id` type change (`String` → `Option<String>`). This is consistent with Plan 01's D-02 decision, which cites MEMORY.md's "v2.0 cleanup philosophy — during breaking-change window, consolidate aggressively." CHANGELOG flags it as **BREAKING (minor-within-v2.x window)** with a copy-pasteable migration snippet for external callers.
- **`mcp-tester 0.5.1 → 0.5.2` is a patch bump**, not a minor. mcp-tester gained zero new public API this phase — only the internal `create_oauth_middleware` call site was updated to the new `OAuthConfig` shape under Plan 01. Patch is the right semver for a transparent consumer bump.
- **`mcp-preview`, `pmcp-macros`, `pmcp-widget-utils` NOT bumped.** None of them have behavior changes this phase. Per CLAUDE.md "Version Bump Rules — Only bump crates that have changed since their last publish."
- **Range-pinned consumers left alone.** `pmcp-macros` uses `>=1.20.0`; `pmcp-code-mode` and `pmcp-code-mode-derive` use `>=2.2.0`. Both already encompass 2.5.0 and express intentional minimum-version tolerances for backward compatibility with older pmcp versions. No change needed.

## Deviations from Plan

**None — plan executed exactly as written.**

The plan's action prose was extremely specific (grep gates + exact line numbers + explicit pin list), so there was no ambiguity. The closing grep gate (`grep -rnE 'pmcp = \{ version = "2\.[234]\.0"' --include=Cargo.toml .`) confirmed all 8 pins were updated. `make quality-gate` passed on the first invocation with no lint / fmt / test drift introduced by the version bumps.

**Total deviations: 0.** **Impact: none — plan shipped exactly the intended source state.**

## Issues Encountered

None. Every gate, grep check, and test suite in the plan's `<acceptance_criteria>` and `<verification>` sections passed on first run.

## Threat Flags

None. This plan is pure release coordination (Cargo.toml + CHANGELOG edits); no new security-relevant surface introduced.

## Operator Next Step

Phase 74 source-code state is release-ready. The operator drives the tag + publish per CLAUDE.md Release & Publish Workflow:

```bash
# 1. Verify HEAD matches release-ready main
git checkout main && git pull

# 2. Tag the release (ONE tag covers all bumped crates: pmcp 2.5.0, cargo-pmcp 0.9.0, mcp-tester 0.5.2)
git tag -a v2.5.0 -m "pmcp v2.5.0 + cargo-pmcp 0.9.0 + mcp-tester 0.5.2 — Phase 74: DCR (RFC 7591) + cargo pmcp auth command group"

# 3. Push the tag — .github/workflows/release.yml fires automatically:
#    - Creates GitHub Release from CHANGELOG.md
#    - Publishes pmcp-widget-utils -> pmcp -> mcp-tester -> mcp-preview -> cargo-pmcp to crates.io
#    - Publishes to the MCP Registry
#    - Attaches cross-platform mcp-tester binaries
git push upstream v2.5.0
```

## Phase 74 Overall Status

All three waves shipped and verified:

| Wave | Plan | Requirement | Status | Key deliverable |
| --- | --- | --- | --- | --- |
| 1 | 74-01 | SDK-DCR-01 | ✅ Complete | RFC 7591 DCR auto-fire in `OAuthHelper` + `AuthorizationResult` + `authorize_with_details()` |
| 2 | 74-02 | CLI-AUTH-01 | ✅ Complete | `cargo pmcp auth {login,logout,status,token,refresh}` + `~/.pmcp/oauth-cache.json` |
| 3 | 74-03 | (release) | ✅ Complete | pmcp 2.5.0 + cargo-pmcp 0.9.0 + mcp-tester 0.5.2 source state on main |

**Phase 74 is ready for the operator's tag + publish.**

## Self-Check: PASSED

**File existence (modified Cargo.toml & CHANGELOG.md are tracked in git):**

```
git show --stat 87a1d100 | head -15
```

Shows 9 files changed, 12 insertions(+), 12 deletions(-) — confirms the trio of version bumps + 8 pin updates + CHANGELOG date.

**Commit existence:**
- `87a1d100` Task 3.1 (version bumps + pin audit): FOUND in `git log --oneline -5`

**Acceptance criteria spot-check (re-run at SUMMARY time):**

- G27 Cargo.toml pmcp 2.5.0: `grep -c '^version = "2.5.0"' Cargo.toml` → `1` ✅
- G27 cargo-pmcp 0.9.0: `grep -c '^version = "0.9.0"' cargo-pmcp/Cargo.toml` → `1` ✅
- G27 pmcp pin 2.5.0 in cargo-pmcp: `grep -c 'pmcp = { version = "2.5.0"' cargo-pmcp/Cargo.toml` → `1` ✅
- CHANGELOG date: `grep -c '^## \[2.5.0\] - 2026-04-21' CHANGELOG.md` → `1` ✅
- Blocker #3 closing grep: `grep -rn 'pmcp = { version = "2.[234].0"' --include=Cargo.toml .` → 0 hits ✅
- mcp-tester 0.5.2: `grep -c '^version = "0.5.2"' crates/mcp-tester/Cargo.toml` → `1` ✅
- G28 `make quality-gate`: exit 0, wall-clock 461 s ✅

All 11 must_haves truths + 3 must_haves artifacts satisfied. SDK-DCR-01 and CLI-AUTH-01 are both end-to-end release-ready.

---
*Phase: 74-add-cargo-pmcp-auth-subcommand-with-multi-server-oauth-token*
*Completed: 2026-04-21*
