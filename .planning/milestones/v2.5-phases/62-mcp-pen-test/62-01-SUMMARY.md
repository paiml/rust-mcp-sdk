---
phase: 62-mcp-pen-test
plan: 01
subsystem: testing
tags: [pentest, security, sarif, governor, rate-limiter, mcp-tester]

# Dependency graph
requires: []
provides:
  - "Pentest module foundation: types (Severity, AttackCategory, SecurityFinding, AttackSurface)"
  - "PentestConfig with fail_on threshold, destructive flag, rate limit, category filter"
  - "PentestRateLimiter wrapping governor GCRA rate limiter"
  - "SecurityReport with JSON serialization and colored terminal display"
  - "SARIF 2.1.0 converter with partialFingerprints for GitHub Security tab"
  - "PayloadLibrary with 9 curated prompt injection payloads"
  - "Attack surface discovery via ServerTester"
  - "PentestEngine orchestrating discovery and stub attack runners"
  - "CLI command: cargo pmcp pentest <url> with all D-05 through D-12 flags"
affects: [62-02-PLAN, 62-03-PLAN]

# Tech tracking
tech-stack:
  added: [serde-sarif 0.8, governor 0.10, sha2 0.10]
  patterns: [domain-based attack runner, rate-limited execution, SARIF output]

key-files:
  created:
    - cargo-pmcp/src/pentest/types.rs
    - cargo-pmcp/src/pentest/config.rs
    - cargo-pmcp/src/pentest/rate_limiter.rs
    - cargo-pmcp/src/pentest/report.rs
    - cargo-pmcp/src/pentest/sarif.rs
    - cargo-pmcp/src/pentest/discovery.rs
    - cargo-pmcp/src/pentest/payloads/mod.rs
    - cargo-pmcp/src/pentest/payloads/injection.rs
    - cargo-pmcp/src/pentest/attacks/mod.rs
    - cargo-pmcp/src/pentest/attacks/prompt_injection.rs
    - cargo-pmcp/src/pentest/attacks/tool_poisoning.rs
    - cargo-pmcp/src/pentest/attacks/session_security.rs
    - cargo-pmcp/src/pentest/engine.rs
    - cargo-pmcp/src/commands/pentest.rs
    - cargo-pmcp/src/pentest/mod.rs
  modified:
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/src/lib.rs
    - cargo-pmcp/src/main.rs
    - cargo-pmcp/src/commands/mod.rs

key-decisions:
  - "Worktree lacks AuthFlags/flags.rs (older version) -- adapted PentestCommand with inline --api-key flag instead of flattened AuthFlags"
  - "Severity derives Hash for use as HashMap key in summary_counts"
  - "SARIF fingerprints use SHA-256 of finding_id:endpoint:severity for deterministic deduplication"
  - "All 19 attack rule IDs defined in SARIF tool component even though attack logic is stubbed"
  - "Attack stubs return empty Vec<SecurityFinding> -- implementation deferred to Plans 02 and 03"

patterns-established:
  - "Domain-based attack runner: each attack category is a separate module returning Vec<SecurityFinding>"
  - "Rate-limited execution: all attack modules receive PentestRateLimiter as parameter"
  - "from_str_loose pattern: case-insensitive parsing with aliases for CLI enum values"

requirements-completed: []

# Metrics
duration: 63min
completed: 2026-03-28
---

# Phase 62 Plan 01: Pentest Module Foundation Summary

**Pentest module with types, config, governor rate limiter, JSON/SARIF reporting, curated injection payloads, and `cargo pmcp pentest` CLI command skeleton**

## Performance

- **Duration:** 63 min
- **Started:** 2026-03-28T13:23:33Z
- **Completed:** 2026-03-28T14:26:31Z
- **Tasks:** 2
- **Files modified:** 19

## Accomplishments

- Created complete pentest type system: Severity (5-level with Ord), AttackCategory (3 MCP-specific categories), SecurityFinding (serializable to JSON with duration_millis helper), AttackSurface with ToolInfo/ResourceInfo/PromptInfo
- Created SARIF 2.1.0 converter with partialFingerprints (SHA-256 based) satisfying GitHub Security tab requirements; includes all 19 attack rule IDs
- Created CLI command `cargo pmcp pentest <url>` with --fail-on, --format (text/json/sarif), --output, --rate-limit, --destructive, --category, --timeout, --transport, --api-key flags
- 47 unit tests covering config threshold logic, severity ordering, JSON round-trips, SARIF output validation, payload library verification

## Task Commits

Each task was committed atomically:

1. **Task 1: Types, config, rate limiter, report, SARIF** - `ed2b89f` (feat)
2. **Task 2: Discovery, payloads, attack stubs, engine, CLI** - `7347f01` (feat)

## Files Created/Modified

- `cargo-pmcp/Cargo.toml` - Added serde-sarif, governor, sha2 dependencies
- `cargo-pmcp/src/pentest/mod.rs` - Module declarations and re-exports
- `cargo-pmcp/src/pentest/types.rs` - Severity, AttackCategory, SecurityFinding, AttackSurface types
- `cargo-pmcp/src/pentest/config.rs` - PentestConfig with fail_on, rate_limit, destructive, categories
- `cargo-pmcp/src/pentest/rate_limiter.rs` - Governor GCRA rate limiter wrapper
- `cargo-pmcp/src/pentest/report.rs` - SecurityReport with JSON and terminal output
- `cargo-pmcp/src/pentest/sarif.rs` - SARIF 2.1.0 converter with 19 rule definitions
- `cargo-pmcp/src/pentest/discovery.rs` - Attack surface discovery from ServerTester
- `cargo-pmcp/src/pentest/payloads/mod.rs` - PayloadLibrary struct
- `cargo-pmcp/src/pentest/payloads/injection.rs` - 9 curated prompt injection payloads
- `cargo-pmcp/src/pentest/attacks/mod.rs` - Attack module declarations
- `cargo-pmcp/src/pentest/attacks/prompt_injection.rs` - Stub returning empty findings
- `cargo-pmcp/src/pentest/attacks/tool_poisoning.rs` - Stub returning empty findings
- `cargo-pmcp/src/pentest/attacks/session_security.rs` - Stub returning empty findings
- `cargo-pmcp/src/pentest/engine.rs` - PentestEngine orchestrating discovery and attacks
- `cargo-pmcp/src/commands/pentest.rs` - PentestCommand clap Args with execute
- `cargo-pmcp/src/commands/mod.rs` - Added pentest module declaration
- `cargo-pmcp/src/main.rs` - Added Pentest variant and mod pentest
- `cargo-pmcp/src/lib.rs` - Added pentest module for library access

## Decisions Made

- **Adapted for worktree version**: This worktree lacks AuthFlags/flags.rs (Phase 29 features). Created PentestCommand with inline `--api-key` flag instead of flattened AuthFlags. The pattern is compatible and can be migrated when merged to main.
- **Severity derives Hash**: Needed for use as HashMap key in summary_counts(). Added Hash to derive list alongside PartialEq, Eq, Serialize, Deserialize.
- **SARIF fingerprints use SHA-256**: `primaryLocationLineHash` computed from `finding_id:endpoint:severity` string. Deterministic and unique per finding instance.
- **All 19 attack rule IDs pre-defined**: SARIF tool component includes PI-01..PI-07, TP-01..TP-06, SS-01..SS-06 as ReportingDescriptors even though attack implementations are stubbed. This ensures SARIF rule references are valid from day one.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Worktree missing AuthFlags infrastructure**
- **Found during:** Task 2 (CLI command creation)
- **Issue:** Worktree is based on an older commit without `flags.rs`, `auth.rs`, or `AuthFlags`. The plan assumes these exist.
- **Fix:** Created PentestCommand with inline `--api-key` CLI flag and passed it directly to ServerTester::new(). Pattern is compatible with future AuthFlags migration.
- **Files modified:** cargo-pmcp/src/commands/pentest.rs
- **Verification:** `cargo pmcp pentest --help` shows all flags including --api-key
- **Committed in:** 7347f01

**2. [Rule 1 - Bug] chrono DateTime timezone mismatch in report.finalize()**
- **Found during:** Task 1 (report module compilation)
- **Issue:** `DateTime::parse_from_rfc3339` returns `DateTime<FixedOffset>`, not `DateTime<Utc>`. Subtraction from `Utc::now()` failed with `Sub<DateTime<FixedOffset>>` not implemented.
- **Fix:** Added `.with_timezone(&chrono::Utc)` conversion after parsing.
- **Files modified:** cargo-pmcp/src/pentest/report.rs
- **Verification:** Compilation succeeds, report.finalize() works correctly in tests
- **Committed in:** ed2b89f

**3. [Rule 1 - Bug] Severity missing Hash derive**
- **Found during:** Task 1 (report module compilation)
- **Issue:** `HashMap<Severity, usize>` in summary_counts() requires Severity to implement Hash.
- **Fix:** Added `Hash` to Severity's derive list.
- **Files modified:** cargo-pmcp/src/pentest/types.rs
- **Verification:** summary_counts test passes
- **Committed in:** ed2b89f

---

**Total deviations:** 3 auto-fixed (1 blocking, 2 bugs)
**Impact on plan:** All fixes necessary for compilation and correctness. No scope creep. The AuthFlags adaptation is forward-compatible.

## Issues Encountered

None beyond the auto-fixed deviations above.

## User Setup Required

None - no external service configuration required.

## Known Stubs

The attack runner modules are intentionally stubbed (returning empty `Vec<SecurityFinding>`):
- `cargo-pmcp/src/pentest/attacks/prompt_injection.rs` - Stub, implemented in Plan 02
- `cargo-pmcp/src/pentest/attacks/tool_poisoning.rs` - Stub, implemented in Plan 02
- `cargo-pmcp/src/pentest/attacks/session_security.rs` - Stub, implemented in Plan 03

These stubs are expected per the plan objective: "The attack runners should be stubs returning empty Vec<SecurityFinding> -- Wave 2 fills them in."

## Next Phase Readiness

- All types, interfaces, and contracts are defined and tested
- CLI command is fully functional (shows help, parses all flags, routes output)
- Plans 02 and 03 can implement attack logic directly against stable SecurityFinding/AttackSurface/PentestConfig interfaces
- PayloadLibrary provides curated injection payloads ready for Plan 02's prompt injection runner

---
*Phase: 62-mcp-pen-test*
*Completed: 2026-03-28*
