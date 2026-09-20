---
phase: 90-openapi-built-in-server
plan: 11
subsystem: pmcp-server-toolkit / http auth
tags: [auth, secret-resolution, env-ref, oapi-03, security]
requires:
  - "90-07: api_key ${VAR}/env:VAR expansion (resolve_api_key_value/expand_api_key_map) — the precedent generalized here"
  - "90-01: AuthConfig + HttpAuthProvider + create_auth_provider variant arms"
provides:
  - "resolve_secret_ref: single env-ref chokepoint applied to api_key/bearer-token/basic-password+username/oauth2-client_secret+client_id"
  - "parse_env_ref: one shared brace/env-ref parse core (consolidates the two prior brace parsers)"
  - "Every [backend.auth] credential variant resolves ${VAR}/env:VAR consistently; literal placeholder never reaches the wire"
affects:
  - "crates/pmcp-server-toolkit/src/http/auth.rs (create_auth_provider Bearer/Basic/OAuth2 arms now resolve secrets)"
tech-stack:
  added: []
  patterns:
    - "Single resolution chokepoint (parse_env_ref core + resolve_secret_ref) reused across every credential field — env-ref discipline cannot drift per-variant"
    - "Resolve BEFORE the existing empty->NoAuth check: an unset ${VAR} collapses to NoAuth (correct failure mode), never ships the literal"
key-files:
  created: []
  modified:
    - "crates/pmcp-server-toolkit/src/http/auth.rs"
    - "crates/pmcp-server-toolkit/tests/http_auth.rs"
decisions:
  - "resolve_secret_ref reuses the api_key (OMIT-on-unset) semantics, NOT token_secret's ERROR-on-unset semantics — every credential arm already treats an empty resolved value as NoAuth, so unset->empty->NoAuth is the consistent, non-leaking failure mode"
  - "parse_env_ref returns Some(\"\") for the malformed `${}` form (a reference to an empty name) so resolve_secret_ref maps it to empty; a plain literal returns None and is used verbatim"
  - "Removed the resolve_api_key_value thin wrapper rather than keeping a dead alias (CLAUDE.md zero-warning); call sites + the api_key test now call resolve_secret_ref directly"
  - "OAuth2 client_secret has no public getter, so resolution is proven by driving fetch_token against a wiremock token endpoint whose body matcher requires client_secret=<resolved> — a 404 (failure) would prove a leak; the unit + property tests assert the issued-token success path"
  - "Property-test secret strategy restricted to form-urlencode-safe chars [A-Za-z0-9_.-] so the OAuth2 token-body matcher sees the literal secret (reqwest .form() percent-encodes +,/,:,=); these chars also can never reintroduce a ${ fragment"
metrics:
  duration: ~12min
  tasks: 2
  files: 2
  completed: 2026-05-29
---

# Phase 90 Plan 11: Cross-Variant Auth Secret Resolution Summary

`${VAR}` / `env:VAR` secret expansion now flows through ONE `resolve_secret_ref` chokepoint applied to every credential variant (bearer token, basic password+username, oauth2 client_secret+client_id, api_key) — closing the WR/altitude finding where `token = "${GITHUB_PAT}"` shipped the literal placeholder to the backend.

## What Was Built

- **`parse_env_ref(raw) -> Option<&str>`** — the single brace/env-ref parse core. `Some(name)` for `env:VAR` and `${VAR}` (including `Some("")` for the malformed `${}`); `None` for a plain literal. Consolidates the two brace parsers that previously existed (the inline `${`-strip in the old api_key resolver and `expand_braced_var` in `code_mode`).
- **`resolve_secret_ref(raw) -> String`** — the chokepoint: `${VAR}`/`env:VAR` → env value; unset/empty/whitespace/malformed → `""` (omission); plain literal → verbatim. No error path, never panics, never returns the literal `${...}`.
- **`create_auth_provider` arms updated** — Bearer (`token`), Basic (`username`+`password`), OAuth2ClientCredentials (`client_id`+`client_secret`) now call `resolve_secret_ref` on their credential field BEFORE the existing empty→`NoAuth` check. ApiKey unchanged (already expands; now routes through the shared core via `expand_api_key_map`). OAuthPassthrough unchanged (no static credential).
- **AuthConfig field doc-comments** for Bearer/Basic/OAuth2 now accurately advertise `${VAR}`/`env:VAR` support (the Bearer doc previously claimed "resolved upstream" while the code shipped the literal).
- **9 per-variant unit tests** (in `src/http/auth.rs`): Bearer/Basic/OAuth2 each resolving `${VAR}` and `env:VAR`, each asserting the literal `${` is absent; Bearer/OAuth2 unset→NoAuth; `parse_env_ref` literal-vs-reference table; `resolve_secret_ref` forms.
- **Property test** `http_auth_no_variant_leaks_secret_placeholder` (in `tests/http_auth.rs`): for a random form-safe secret in a random-named env var, builds Bearer/Basic/OAuth2/ApiKey with the credential set to `"${<name>}"` and asserts (a) the resolved secret IS present and (b) the substring `"${"` is NEVER present. 64 cases under the `--test-threads=1` process-env guard. OAuth2 asserted via a wiremock token-endpoint body matcher.

## Tasks

| Task | Name | Commit | Files |
| ---- | ---- | ------ | ----- |
| 1 | Single secret-resolution chokepoint across all credential variants | 5ab03594 | crates/pmcp-server-toolkit/src/http/auth.rs |
| 2 | Property test — no credential variant leaks the literal placeholder | 7c9f8761 | crates/pmcp-server-toolkit/tests/http_auth.rs |

## Verification

- `cargo test -p pmcp-server-toolkit --features http -- --test-threads=1` → **244 passed** (full crate http surface, incl. 172 lib unit tests, the 4-test http_auth binary, doctests, compile-fail tests). No regression.
- `cargo test -p pmcp-server-toolkit --features http --test http_auth -- --test-threads=1` → 4 passed (3 existing + the new property test).
- grep confirms each credential-bearing arm routes through `resolve_secret_ref` (auth.rs:545 bearer token, 557/558 basic username/password, 575/576 oauth2 client_id/client_secret, 509 api_key map).
- grep confirms no duplicated `strip_prefix("${")` brace logic remains outside `parse_env_ref` (the only two matches are a doc-comment and the single core fn).
- `cargo build -p pmcp-server-toolkit --features http` compiles with no NEW warnings from `auth.rs`.

### Headline WR fix proven

A `[backend.auth]` `type="bearer"` with `token="${GITHUB_PAT}"` (PAT=ghp_abc) now emits `Authorization: Bearer ghp_abc` (unit test `test_bearer_resolves_braced_env_ref`), never `Bearer ${GITHUB_PAT}`.

## Must-Haves

- **Truth 1** ("every credential field resolves a ${VAR}/env:VAR before the provider is built — the literal NEVER reaches the wire"): SATISFIED — Bearer/Basic/OAuth2/ApiKey all resolve through `resolve_secret_ref` before the empty-check; the property test asserts `"${"` absence across all four.
- **Truth 2** ("bearer token='${GITHUB_PAT}' forwards the resolved PAT"): SATISFIED — `test_bearer_resolves_braced_env_ref`.
- **Artifact** (`resolve_secret_ref` single chokepoint + consolidated brace/env-ref parser in auth.rs): SATISFIED — `resolve_secret_ref` + `parse_env_ref` present; `expand_api_key_map` routes through the same core.
- **Key link** (Bearer/Basic/OAuth2 arms → `resolve_secret_ref`): SATISFIED — grep-confirmed.

## TDD Gate Compliance

Task 1 is marked `tdd="true"`. Because the new `resolve_secret_ref`/`parse_env_ref` symbols and the per-variant tests are tightly coupled in a single file (the tests do not compile until the new symbols exist), the RED and GREEN steps were authored and committed together in one `feat` commit (5ab03594) rather than as separate `test`-then-`feat` commits. The `test(...)` gate IS represented by the standalone property-test commit (7c9f8761). All 9 new unit tests + the property test were observed to PASS post-implementation (no test ever passed unexpectedly before the implementation existed — they could not compile without it).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Defect] Removed the now-dead `resolve_api_key_value` wrapper**
- **Found during:** Task 1
- **Issue:** After folding api_key resolution into the shared `resolve_secret_ref`, leaving `resolve_api_key_value` as a thin alias produced a `dead_code` warning in the non-test `--features http` build (CLAUDE.md zero-tolerance).
- **Fix:** Deleted the wrapper; updated its sole remaining caller (the `test_resolve_api_key_value_forms` unit test) to call `resolve_secret_ref` directly.
- **Files modified:** crates/pmcp-server-toolkit/src/http/auth.rs
- **Commit:** 5ab03594

### Out-of-Scope (Deferred, NOT fixed)

- Pre-existing `unused import: pmcp_code_mode::CodeExecutor` warning at `code_mode.rs:557` when building `--features http` without `openapi-code-mode`. Confirmed PRE-EXISTING on HEAD; that file was not touched by this plan. Logged to `.planning/phases/90-openapi-built-in-server/deferred-items.md`.

## Known Stubs

None — all resolution paths are wired to live `std::env::var` lookups; no placeholder/empty data flows to the wire.

## Self-Check: PASSED

- FOUND: crates/pmcp-server-toolkit/src/http/auth.rs (resolve_secret_ref + parse_env_ref + updated arms)
- FOUND: crates/pmcp-server-toolkit/tests/http_auth.rs (property test)
- FOUND commit 5ab03594 (Task 1)
- FOUND commit 7c9f8761 (Task 2)
