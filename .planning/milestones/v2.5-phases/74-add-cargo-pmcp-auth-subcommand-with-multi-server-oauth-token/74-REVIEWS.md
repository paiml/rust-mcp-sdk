---
phase: 74
reviewers: [codex, gemini]
reviewed_at: 2026-04-21
plans_reviewed: [74-01-PLAN.md, 74-02-PLAN.md, 74-03-PLAN.md]
self_cli_skipped: claude
notes: >
  Cross-AI peer review of Phase 74's 3-plan breakdown (SDK DCR + CLI auth group
  + release coordination). Codex gave MEDIUM overall risk with APPROVE WITH
  REVISIONS (1 HIGH, 5 MEDIUM, 4 LOW). Gemini gave LOW overall risk with APPROVED
  (0 HIGH, 1 MEDIUM, 2 LOW). Codex's HIGH finding on missing `response_types:
  ["code"]` in the DCR request body is a real RFC 7591 §3.1 interop bug that
  Gemini missed — if unaddressed it will cause DCR to fail against the
  authoritative pmcp.run parser. Codex also flagged a scope-relevant concern
  around `authorize_with_details()` not covering the device-code fallback path,
  and a real concurrency concern that the current last-writer-wins cache can
  drop UNRELATED entries (not just same-URL races). See Consensus Summary below.
---

# Cross-AI Plan Review — Phase 74

## Codex Review

# Phase 74 Plan Review

## Overall Summary
The phase is well-scoped and the dependency order is mostly right: Plan 01 establishes SDK DCR, Plan 02 consumes it in `cargo-pmcp`, and Plan 03 closes release hygiene. The strongest parts are the explicit guardrails, concrete verification steps, and the separation between SDK and CLI concerns. The main risks are not conceptual; they are edge-case and completeness risks: the DCR wire shape is slightly off for the authoritative `pmcp.run` parser, the `client_id: String -> Option<String>` migration needs one more repo-wide audit pass, `authorize_with_details()` does not fully specify the device-code fallback path, and the cache write strategy can drop unrelated entries under parallel logins.

### Plan 01: SDK DCR

**Summary.** Good plan overall. It hits the right abstraction boundary by putting DCR in `pmcp::client::oauth` instead of cargo-pmcp, and it has serious test intent. The main issue is that the planned DCR request body does not quite match the effective contract of the current `pmcp.run` registration endpoint.

**Strengths.**
- Correctly places DCR in the SDK, which matches the phase goal and avoids CLI-only coupling.
- The trigger conditions for auto-DCR are clear and sane: `dcr_enabled && client_id.is_none() && registration_endpoint.is_some()`.
- The breaking `OAuthConfig` change is acknowledged explicitly, with migration notes and workspace build verification.
- Good security posture on T-74-A and T-74-C: HTTPS guard plus parser robustness testing.
- Exporting `AuthorizationResult` is the right move; Plan 02 needs real `refresh_token`, `expires_at`, `issuer`, and `client_id`.

**Concerns.**
- **HIGH**: The DCR request builder uses `response_types: vec![]`, and `DcrRequest` skips empty vectors during serialization. The current `pmcp.run` parser's `ClientRegistrationRequest` expects `response_types` as a required field. As written, this can fail against the authoritative target. It should send `response_types: ["code"]`.
- **MEDIUM**: `authorize_with_details()` is specified mostly around the authorization-code path. The current SDK falls back to device code flow. The plan does not clearly state how full details are captured on that fallback path.
- **MEDIUM**: The plan's "all in-repo callers updated" claim is mostly strong, but I would not trust the enumerated list alone. The breaking-field migration should be validated with a repo-wide `rg 'OAuthConfig {'` sweep, not just named files.
- **LOW**: The public test hook `test_resolve_client_id_from_discovery()` under `#[cfg(any(test, feature = "oauth"))]` would expose a test-only API in normal `oauth` builds. That is avoidable.
- **LOW**: The localhost exception for T-74-A should probably include IPv6 loopback (`[::1]`), not just `localhost` and `127.0.0.1`.

**Suggestions.**
- Change the DCR body to include `response_types: ["code"]` and add that to the integration-test assertions.
- Make `authorize_with_details()` truly mirror `get_access_token()`, including device-code fallback, or document explicitly that Phase 74 only supports full-details capture on auth-code servers.
- Replace the planned caller list as the primary safety mechanism with a repo-wide grep gate over `OAuthConfig {`.
- Avoid a public test hook in release builds; prefer an internal helper exercised by unit tests, or a dedicated test-only feature.
- Add `[::1]` to the loopback allowlist.

**Risk Assessment.** MEDIUM. The design is correct, but the `response_types` omission is a real interoperability bug, not a cosmetic issue.

### Plan 02: CLI auth group

**Summary.** This is a solid CLI plan. The command surface is coherent, the cache fallback is placed in the right layer, and the precedence model matches user expectations from tools like `gh` and AWS CLIs: explicit input wins over implicit state. The main remaining question is whether the cache write strategy is acceptable without locking.

**Strengths.**
- `flag > env > cache` is the right precedence order and aligns with common CLI patterns.
- `--client` being login-only and mutually exclusive with `--oauth-client-id` is correct. For `pmcp.run`, `client_name` drives DCR-time branding/classification; pairing it with a fixed `client_id` is contradictory.
- Keeping cache lookup out of `AuthFlags::resolve()` and instead adding it in `resolve_auth_middleware` / `resolve_auth_header` is the right layering.
- Token leak prevention is mostly well thought through: `auth login` does not print the token, while `auth token` is intentionally raw-stdout only.
- The cache schema is reasonable for v1: explicit `schema_version`, tolerant future extension via serde's unknown-field behavior, and deterministic JSON via ordered maps.

**Concerns.**
- **MEDIUM**: The cache key is `scheme://host[:port]` only. That is an explicit decision, but it means different MCP endpoints on the same host/path collide. If a server hosts multiple auth realms behind paths, users can overwrite tokens.
- **MEDIUM**: Last-writer-wins atomic rename is acceptable for same-key races, but it is lossy for different-key races too. Two concurrent logins to different servers can still drop one entry because the whole file is rewritten from a stale read.
- **MEDIUM**: Exposing `pub mod commands;` from `cargo-pmcp/src/lib.rs` just to support integration tests broadens the public API more than necessary.
- **LOW**: The plan relies on `HOME` overrides in integration tests. That is fine on Unix, but Windows may need a fallback to `USERPROFILE` or conditional gating.
- **LOW**: Token leak prevention is good in CLI output, but I would still grep for `tracing::*` with token-bearing values in the new code paths before shipping.

**Suggestions.**
- If you want to keep v1 simple, accept last-writer-wins but document clearly that parallel logins may lose entries. If you can afford a little more complexity, add advisory file locking or a read-merge-retry loop.
- Consider keying v1 as `scheme://host[:port] + path` unless there is a strong product reason not to. The current choice is easy, but it is the main long-term schema limitation.
- Prefer a narrower public test seam over exporting the whole `commands` tree from the library.
- Add one explicit integration test for "explicit API key overrides cached OAuth token" since that is central to D-13.
- Add a grep gate ensuring `auth login` and `auth status` never print `access_token` or `refresh_token`.

**Risk Assessment.** MEDIUM. The UX and precedence are good. The main unresolved tradeoff is the cache concurrency story.

### Plan 03: Release coordination

**Summary.** Pragmatic and necessary. The version/pin audit is real, not theoretical; the current workspace still has multiple exact `pmcp = "2.2.0"` pins and stale crate versions. The only notable weakness is that the audit text has a counting inconsistency, which suggests it needs one final cleanup pass before execution.

**Strengths.**
- The pin audit is directionally correct and clearly needed; the current workspace still has stale exact-version pins.
- Bumping `mcp-tester` is sensible because it consumes the changed SDK API.
- Using `make quality-gate` instead of ad hoc cargo commands is the right release discipline.
- Separating source-state release prep from actual tagging/publishing is a good operator boundary.

**Concerns.**
- **MEDIUM**: The plan says "7 pins across 6 files" but then enumerates 8 pins across 7 files. The enumeration looks more trustworthy than the summary, but the mismatch should be fixed.
- **LOW**: `rustup update stable` may be undesirable in some local or CI environments; it is operationally fine, but it is not required to validate the source state.
- **LOW**: The release plan assumes no additional lockfile or workspace metadata drift; in practice, the quality gate will catch most of that, but it is worth calling out.

**Suggestions.**
- Fix the pin-count/file-count inconsistency in the plan text before execution.
- Treat `rustup update stable` as optional preflight rather than a hard step unless local policy requires it.
- Add one final grep gate for stale `2.2.0`, `2.4.0`, `0.8.1`, and `0.5.1` literals, then trust `cargo check --workspace --features full`.

**Risk Assessment.** LOW. Mostly mechanical work, and the current repo state supports the need for it.

### Direct Answers To The Review Checks
- Plan 01 RFC 7591 wire format: almost, but not quite. The missing `response_types` field is the main issue; set it to `["code"]`.
- `OAuthConfig::client_id` breaking change: mostly handled safely, but I would require a repo-wide `OAuthConfig {` audit in addition to the named file list.
- Plan 02 cache precedence: yes, it aligns with common CLI behavior.
- `--client` mutex with `--oauth-client-id`: yes, that makes sense and is the correct behavior for `pmcp.run`'s DCR classifier.
- Plan 03 pin audit: substantively complete, but the counts in the prose are inconsistent and should be corrected.
- Threats T-74-A..G: A, C, D, E, G are concretely mitigated; B is mostly okay but the PKCE rationale is sloppy; F is accepted rather than mitigated.
- Token leak prevention: `auth login` is designed not to print the token, and `auth token` is intentionally stdout-only. I would still add grep-based leak checks.
- Cache forward-compat: acceptable for v1. `schema_version` is fine. The real limitation is the cache key, not the JSON shape.
- Concurrent login race: acceptable if you explicitly accept possible entry loss in rare parallel use, but I would prefer lightweight locking or merge-retry if you want this to feel production-grade.

**Bottom Line.** Approve with revisions. Plan 01 needs the `response_types` fix before I'd call it safe. Plan 02 is good but should either embrace or harden the concurrent-write tradeoff explicitly. Plan 03 is ready after cleaning up the pin-count inconsistency.

---

## Gemini Review

### Summary

The Phase 74 plan set is exceptionally high-quality, demonstrating rigorous adherence to the "Toyota Way" standards and project-specific security mandates. By moving Dynamic Client Registration (DCR) into the SDK (Plan 01) and building the multi-server cache at the CLI layer (Plan 02), the architecture correctly balances library flexibility with CLI developer experience. The handling of the `OAuthConfig` breaking change is well-reasoned for a pre-1.0 project, and the inclusion of fuzz, property, and integration tests for every new surface ensures long-term stability.

### Strengths

- **RFC Compliance**: Plan 01's DCR implementation correctly mirrors RFC 7591 §3.1 requirements by reusing existing server-side types, ensuring wire-compatibility with `pmcp.run`.
- **Security Discipline**: The mitigation strategy for T-74-E (Token Leakage) is thorough, using grep gate G20 to prevent accidental `println!` of access tokens and adopting the `gh`-style stdout-only discipline for `auth token`.
- **Pragmatic Concurrency**: The choice of a lock-free "last-writer-wins" strategy via `tempfile::NamedTempFile::persist` is the standard for developer tools (like `aws` or `gh`) and avoids the complexity and platform-specific bugs of cross-process file locking.
- **Comprehensive Pin Audit**: Plan 03 includes a specific list of 7 workspace-internal pins across 6 files, preventing the common "stale dependency" bugs that plague workspace releases.
- **Actionable Error Paths**: The plan explicitly dictates "actionable error messages" (e.g., G5) that tell the user exactly how to fix the problem (provide a manual ID or check TLS), reducing support burden.

### Concerns

- **Breaking Change Churn (MEDIUM)**: Changing `client_id: String → Option<String>` in `OAuthConfig` is a source-level breaking change. While acceptable in the v2.x window, it may break external SDK consumers not present in this workspace. *Disposition*: Mitigated by the additive CHANGELOG migration section and the clear rationale that DCR is the primary async pattern forward.
- **Pentest CI Regressions (LOW)**: The migration of `pentest.rs` to shared `AuthFlags` (D-21) expands its flag surface. There is a small risk that existing CI pipelines using `pentest` with environmental `MCP_API_KEY` might behave differently if they happen to also have a stale `~/.pmcp/oauth-cache.json` present. *Disposition*: The precedence order (flag > env > cache) preserves the behavior of current users.
- **Refresh Token Nullability (LOW)**: As noted in Pitfall 5, some IdPs omit refresh tokens. *Disposition*: Plan 01 Task 1.2b and Plan 02 Task 2.3 correctly handle `Option<String>` for the refresh token, and the `refresh` command returns an actionable error if it's missing.

### Suggestions

- **Platform-specific perm tests**: Ensure the Unix `0o600` permission tests in Plan 02 are strictly gated with `#[cfg(unix)]` to avoid failure on Windows where ACLs behave differently. (Already included in Task 2.4, but deserves special care during execution.)
- **Normalization Hoisting**: While `normalize_cache_key` is currently CLI-local (Plan 02), consider moving it to a `pub(crate)` helper in the SDK in a future phase if any internal SDK component (like a future client-side metrics logger) needs to group data by server origin.
- **DCR Body Capping**: In `do_dynamic_client_registration`, consider adding a 1MB response size limit to the `reqwest` call to mitigate potential (though unlikely) DoS from a hostile discovery endpoint.

### Risk Assessment

**Overall Risk: LOW.** The phase is primarily "plumbing" of existing, proven primitives. The logic is additive and doesn't touch the protocol's message-passing core or transport layer.
- **Dependencies**: Zero new runtime dependencies (promoting `tempfile` is safe).
- **Compatibility**: Precedence rules protect existing CI/CD usage of API keys.
- **Security**: STRIDE threats are fully addressed with concrete guardrails (0o600 perms, HTTPS-only discovery, no-leak grep gates).

**Verdict: APPROVED.** All 28 guardrails (G1..G28) correctly cover the requirements SDK-DCR-01 and CLI-AUTH-01. The plan is ready for Wave 1 execution.

---

## Consensus Summary

Two reviewers completed. Codex = MEDIUM risk, APPROVE WITH REVISIONS (1 HIGH, 5 MEDIUM, 4 LOW). Gemini = LOW risk, APPROVED (0 HIGH, 1 MEDIUM, 2 LOW). Gemini's pass is notably optimistic — it missed the HIGH-severity RFC 7591 interop bug Codex caught. Codex's review is the stronger anchor.

### Agreed Strengths

Both reviewers highlighted:
- **Correct SDK/CLI layering** — DCR in `pmcp::client::oauth`, cache fallback in `cargo-pmcp` `resolve_auth_middleware` (not `AuthFlags::resolve()`)
- **Precedence model** `flag > env > cache` matches `gh` / `aws` CLI conventions and preserves existing CI behavior
- **`--client` as login-only mutex with `--oauth-client-id`** is correct given pmcp.run's DCR classifier semantics
- **Comprehensive validation** — 28 G-rows + 7 threat refs + CLAUDE.md ALWAYS coverage (fuzz + property + unit + example)
- **`OAuthConfig` breaking change handling** is explicit and well-scoped; CHANGELOG migration note + in-repo caller updates

### Agreed Concerns (priority order)

1. **MEDIUM (Codex) — `OAuthConfig {` repo-wide sweep.** Both reviewers flagged the breaking change as "well-handled." Codex additionally asks for a repo-wide `rg 'OAuthConfig {'` grep gate instead of trusting the enumerated caller list. Gemini noted external-consumer risk but deemed it acceptable in the v2.x window.

2. **MEDIUM (Codex) / LOW (Gemini) — Pentest migration caveats.** Both noted the `pentest.rs` flag migration as a potential regression surface. Mitigation already present (precedence order), but deserves integration-test coverage.

3. **LOW (Codex) — Token leak check coverage.** Codex suggested a `tracing::*` grep gate for token-bearing values (beyond just `println!/eprintln!`). Gemini praised the existing `println!` grep but didn't extend to `tracing::*`.

### Divergent Views

**Codex-only findings (not surfaced by Gemini):**

1. **HIGH — RFC 7591 `response_types` missing.** The DCR request builder serializes `response_types: vec![]` with `#[serde(skip_serializing_if = "Vec::is_empty")]`, which means the field is DROPPED from the wire request. pmcp.run's `ClientRegistrationRequest` expects `response_types` — missing it WILL break DCR against the authoritative target. Gemini claimed "RFC Compliance ✓" but did not verify the serialized wire shape matches the server parser. **This is the single most important finding in the review.**

2. **MEDIUM — `authorize_with_details()` device-code fallback undefined.** The new method specified in Plan 01 Task 1.2b mirrors `get_access_token()` for the authorization-code path, but the existing SDK has a device-code fallback path (`generic_oidc.rs`). The plan does not clearly specify how the full `AuthorizationResult` (refresh_token, expires_at, etc.) is captured on that fallback path.

3. **MEDIUM — Last-writer-wins concurrency is lossy for DIFFERENT-URL races too.** Plan 02 Task 2.1's T-74-F mitigation narrative (post iteration-1 revision) covers same-URL and different-URL races as both last-writer-wins. But Codex argues this is a real correctness bug in practice: two concurrent `auth login` calls for different servers can cause one server's entry to be dropped because both writers read the pre-write cache, both add their own entry in memory, and the second writer's atomic rename overwrites the first's additions. Gemini accepted this tradeoff as "pragmatic concurrency" (matches `aws` / `gh`). This is a judgment call — Codex prefers lightweight locking or read-merge-retry; Gemini prefers the existing lock-free approach.

4. **MEDIUM — `pub mod commands;` broadens public API unnecessarily.** Codex flagged the Blocker #2 fix (exposing the full `commands` tree from `cargo-pmcp/src/lib.rs`) as over-broad — a narrower public test seam would be preferable. Gemini did not notice this.

5. **LOW — Public test hook `test_resolve_client_id_from_discovery()` leaks into release builds.** Codex noted the `#[cfg(any(test, feature = "oauth"))]` guard exposes a test-only API in normal builds with the `oauth` feature enabled. Gemini did not catch this.

6. **LOW — IPv6 loopback `[::1]` missing from T-74-A allowlist.** Codex suggested adding `[::1]` alongside `127.0.0.1` and `localhost`. Gemini did not flag.

7. **LOW — Plan 03 pin-count inconsistency ("7 pins across 6 files" vs enumerated 8 pins across 7 files).** Codex caught a wording drift between the prose summary and the explicit enumeration. Gemini's "7 pins across 6 files" quote suggests it read the summary and not the enumeration.

**Gemini-only findings (not surfaced by Codex):**

1. **LOW — DCR body response size cap.** Gemini suggested adding a 1MB response-size limit on the `reqwest` DCR POST to mitigate DoS from a hostile discovery endpoint. Codex did not suggest this, but it's a reasonable defense-in-depth.

### Recommended Actions

**HIGH priority — MUST fix before execution:**

1. **Codex HIGH — `response_types: ["code"]` in DCR body.** This is a real RFC 7591 compliance bug. In Plan 01 Task 1.2 (DCR request builder), change the request literal from `response_types: vec![]` (or whatever constructor shape is used) to `response_types: vec!["code".to_string()]`. Add to Plan 01 Task 1.3 integration test assertions that the serialized body contains `"response_types":["code"]`. Add grep gate: `grep -c 'response_types.*code' src/client/oauth.rs` returns `>= 1`.

**MEDIUM priority — address via single revision pass:**

2. **Codex — Repo-wide `OAuthConfig {` grep gate** (in addition to the enumerated caller list). Add to Plan 01 Task 1.1 acceptance_criteria: after the struct change, `rg -c 'OAuthConfig \{' --type rust | <expected-count>` — planner establishes the expected count pre-change, then asserts it post-change.

3. **Codex — Document `authorize_with_details()` device-code fallback behavior.** Plan 01 Task 1.2b should explicitly state: either (a) the new method supports both auth-code AND device-code flows with full details captured on both, OR (b) device-code flow falls back to the legacy `get_access_token()` path and the cache entry is incomplete (refresh_token may be None if the device-code IdP doesn't supply one). Pick one and document it.

4. **Codex — Concurrency story for Plan 02.** Two options: (a) accept last-writer-wins, add explicit doc note that "parallel `auth login` calls may lose entries in rare cases; prefer sequential logins during initial setup," OR (b) add lightweight flock-based advisory locking (`fs2::FileExt::try_lock_exclusive`) or a read-merge-retry loop. Option (a) matches `gh`/`aws` conventions; option (b) is more production-grade. Pick one and codify in Plan 02 Task 2.1.

5. **Codex — Narrower public test seam instead of `pub mod commands;`.** Replace the blanket `pub mod commands;` (added in iteration 1 Blocker #2 fix) with a narrower re-export: `pub mod commands { pub mod auth_cmd { pub use super::super::commands::auth_cmd::cache; } }` or similar. Only exposes what the integration test needs.

**LOW priority — batch into the same revision:**

6. **Codex — Avoid test hook leaking into release.** Move `test_resolve_client_id_from_discovery()` from `#[cfg(any(test, feature = "oauth"))]` to `#[cfg(test)]` only, or expose behind a dedicated `test-utils` feature.

7. **Codex — Add `[::1]` to T-74-A loopback allowlist.** Minor hardening.

8. **Codex — `tracing::*` token-leak grep gate.** Extend G20 to include `tracing::(info|debug|warn|error)!` macros, not just `println!/eprintln!`.

9. **Codex — Fix Plan 03 pin-count prose inconsistency** ("7 pins across 6 files" → "8 pins across 7 files" or recount and align).

10. **Codex — Add "explicit API key overrides cached OAuth" integration test.** Plan 02 Task 2.4 should have one test that seeds the cache, then runs `test conformance <url> --api-key <value>` and asserts the `--api-key` value is used (not the cache).

11. **Gemini — DCR response size cap.** Optional defense-in-depth; add `reqwest::Response::bytes_limit(1_000_000)` or equivalent to the DCR POST in Plan 01 Task 1.2.

### Verdict

**Approve with revisions.** None are blocking for the planning phase, but the HIGH finding (#1 — `response_types: ["code"]`) is a real RFC 7591 interop bug that would manifest at execution time against pmcp.run. Priority fix before running `/gsd-execute-phase 74`.

Run `/gsd-plan-phase 74 --reviews` to fold these findings back into the plans. Or, if you accept the current plan and want to fix only the HIGH finding, do a minimal edit to Plan 01 Task 1.2's DCR request body + acceptance criteria without going through the full revision orchestrator.
