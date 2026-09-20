---
phase: 90-openapi-built-in-server
plan: 12
subsystem: config
tags: [openapi, http, auth, oauth_passthrough, config-validation, trust-boundary, thiserror]

# Dependency graph
requires:
  - phase: 90-openapi-built-in-server
    provides: "[backend] section + BackendSection.base_url (Plan 02); OAuthPassthroughAuth relay + create_passthrough_auth_provider (Plans 01/10); resolve_secret_ref chokepoint (Plan 11)"
provides:
  - "ConfigValidationError::EmptyBackendBaseUrl + the validate() check rejecting a present [backend] with empty/omitted base_url at parse time (GAP 3 / WR-02)"
  - "oauth_passthrough trust-boundary documentation at the OAuthPassthroughAuth type, at the relay site, and in both crate READMEs (GAP 5 / WR-04)"
affects: [phase-90-verification, openapi-server-operators]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "http-gated validate() rule: a #[cfg(feature=\"http\")] block in ServerConfig::validate that simply vanishes in no-http builds (SQL configs unaffected)"
    - "Trust-boundary documentation pattern: client-controlled VALUE vs operator-controlled NAME stated at the type doc, the relay site, and the user-facing READMEs"

key-files:
  created: []
  modified:
    - "crates/pmcp-server-toolkit/src/error.rs - ConfigValidationError::EmptyBackendBaseUrl variant"
    - "crates/pmcp-server-toolkit/src/config.rs - validate() http-gated base_url check + 5 tests"
    - "crates/pmcp-server-toolkit/src/http/auth.rs - trust-boundary doc on OAuthPassthroughAuth + relay-site comment"
    - "crates/pmcp-openapi-server/README.md - oauth_passthrough trust-boundary subsection"
    - "crates/pmcp-server-toolkit/README.md - http module entry + oauth_passthrough trust-boundary subsection"

key-decisions:
  - "EmptyBackendBaseUrl message uses [backend].base_url field-naming form to match the existing AmbiguousToolKind/EmptyToolName message style; the literal field token in the message is [backend].base_url (bracketed), so the verification assertion matches that exact substring."
  - "The base_url check is gated #[cfg(feature=\"http\")] because the ServerConfig.backend field is itself http-only; placing it inside the cfg block keeps both no-http and http builds compiling 0 warnings with no dead stub."
  - "GAP 5 is docs-only — no length cap added (WR-04 listed it as 'consider', not required); the HeaderValue::try_from control-char guard remains the sole protection, documented as such."

patterns-established:
  - "http-feature-gated validation rule inside an otherwise feature-independent validate()"
  - "Three-surface trust-boundary documentation (type doc + relay-site code comment + both READMEs)"

requirements-completed: [OAPI-03]

# Metrics
duration: 6min
completed: 2026-05-30
---

# Phase 90 Plan 12: Auth/Config Hardening (GAP 3 + GAP 5) Summary

**`[backend].base_url` now fails fast at config validation with an actionable `EmptyBackendBaseUrl` error instead of a late opaque DispatchError, and the `oauth_passthrough` trust boundary (client controls token value, operator controls target header) is documented at the type, the relay site, and in both crate READMEs.**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-05-30T00:56Z
- **Completed:** 2026-05-30T01:02Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- **GAP 3 / WR-02 closed:** A `[backend]` block with an empty or omitted `base_url` (the field is `#[serde(default)]` → `""`) is now rejected at `ServerConfig::validate()` time via the new `ConfigValidationError::EmptyBackendBaseUrl`, whose message names the field and suggests the fix. Previously this surfaced late and opaquely as `DispatchError::Connector("invalid base URL")` at the first backend request.
- **GAP 5 / WR-04 closed:** The `oauth_passthrough` trust posture is now explicit at three surfaces — the `OAuthPassthroughAuth` type doc-comment (`# Trust boundary (WR-04)`), a `TRUST BOUNDARY` code comment at the `headers.insert` relay site, and an `oauth_passthrough trust boundary` subsection in both `pmcp-openapi-server/README.md` and `pmcp-server-toolkit/README.md`. The boundary: the MCP client controls the forwarded token VALUE; the operator controls the destination header NAME (`target_header`); the `HeaderValue::try_from` control-char rejection is the protection; relaying the client credential is intended SSO passthrough.
- No behavior change for GAP 5 (docs only); GAP 3 is a fast-fail conversion of an existing late failure.

## Task Commits

Each task was committed atomically:

1. **Task 1: Validate non-empty backend.base_url at parse time (GAP 3 / WR-02)** — `7f2ce7b4` (feat) — TDD: RED confirmed via compile-failure (variant absent) before adding the variant + check; GREEN with 5 passing tests. Test and implementation committed together as one feat commit (same two files).
2. **Task 2: Document the oauth_passthrough trust boundary (GAP 5 / WR-04)** — `4b7c5082` (docs)

**Plan metadata:** (this commit) `docs(90-12): complete plan`

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/error.rs` — added `ConfigValidationError::EmptyBackendBaseUrl` with an actionable, field-naming `#[error("[backend].base_url must be non-empty …")]` message + Phase 90 gap-closure doc-comment.
- `crates/pmcp-server-toolkit/src/config.rs` — `validate()` gains an `#[cfg(feature = "http")]` check rejecting a present `[backend]` with empty/whitespace `base_url`; doc rule-list updated (rule 6); 5 new http-gated unit tests (empty reject, omitted reject, non-empty accept, absent-backend accept, error-names-field).
- `crates/pmcp-server-toolkit/src/http/auth.rs` — `OAuthPassthroughAuth` type doc gains a `# Trust boundary (WR-04)` section; relay site (`headers.insert`) gains a `TRUST BOUNDARY (WR-04)` code comment.
- `crates/pmcp-openapi-server/README.md` — `### oauth_passthrough trust boundary (WR-04)` subsection under "Outgoing authentication".
- `crates/pmcp-server-toolkit/README.md` — added the `http` module to the module list and an `### oauth_passthrough trust boundary (WR-04)` subsection.

## Decisions Made

- **Error message field token is bracketed (`[backend].base_url`)** to match the existing message style (`[[tools]]`, `server.name`). The verification assertion checks for that exact substring.
- **Check gated `#[cfg(feature = "http")]`** since the `backend` field is http-only — the block vanishes cleanly in no-http builds; verified 0-warning compile on both `cargo build -p pmcp-server-toolkit` (no-http) and `--features http`.
- **GAP 5 is docs-only** — no length cap added (WR-04 lists it as "consider", not required). The `HeaderValue::try_from` control-char guard stays the sole protection and is documented as such.

## Deviations from Plan

None — plan executed exactly as written.

The pre-existing `unused import: pmcp_code_mode::CodeExecutor` warning in `code_mode.rs:557` (committed by Plan 90-10, `6437eca4`) is OUT OF SCOPE — not in any file this plan touched. Not fixed; noted here only because it appears in build output. The doc-build emits 23 pre-existing rustdoc warnings (verified identical count with and without this plan's changes via `git stash`); this plan introduced zero new doc warnings (the new `[apply]: OAuthPassthroughAuth::apply` intra-doc link resolves cleanly).

## Issues Encountered

- Initial `empty_backend_base_url_error_names_the_field` test asserted `contains("backend.base_url")`, which failed because the actual message uses the bracketed `[backend].base_url` form (the `].` between `backend` and `base_url` breaks the unbracketed substring). Fixed the assertion to `contains("[backend].base_url")` — caught and corrected within the GREEN phase.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- WR-02 and WR-04 are closed; Plan 90-12 was the gap-closure plan for OAPI-03's auth/config surface.
- All 249 toolkit tests pass under `--features http -- --test-threads=1`; `http_auth` suite (4 tests) unchanged.
- No blockers.

## Self-Check: PASSED

- All 5 modified files exist on disk.
- Both task commits (`7f2ce7b4`, `4b7c5082`) exist in git history.
- `EmptyBackendBaseUrl` present in both `error.rs` and `config.rs` (must_have artifact `contains` checks satisfied).

---
*Phase: 90-openapi-built-in-server*
*Completed: 2026-05-30*
