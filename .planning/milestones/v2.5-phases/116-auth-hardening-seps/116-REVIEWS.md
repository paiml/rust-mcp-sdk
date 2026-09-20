---
phase: 116
reviewers: [gemini, codex]
reviewed_at: 2026-08-03T13:30:32Z
plans_reviewed: [116-01-PLAN.md, 116-02-PLAN.md, 116-03-PLAN.md, 116-04-PLAN.md, 116-05-PLAN.md, 116-06-PLAN.md, 116-07-PLAN.md, 116-08-PLAN.md, 116-09-PLAN.md, 116-10-PLAN.md, 116-11-PLAN.md, 116-12-PLAN.md, 116-13-PLAN.md, 116-14-PLAN.md, 116-15-PLAN.md]
---

# Cross-AI Plan Review — Phase 116: Auth Hardening SEPs

Two independent reviewers were given identical prompts containing PROJECT.md context, the
ROADMAP phase section, AUTH-01/02/03 requirement text, 116-CONTEXT.md, 116-VALIDATION.md and
all fifteen PLAN.md files (341 KB total). Neither had repository access — the review is of the
plans as written.

**They reached opposite verdicts.** Gemini approved the phase for execution; Codex rated it
HIGH risk and "unsafe or impossible to execute as written." See Consensus Summary below.

---

## Gemini Review

# Cross-AI Plan Review: Phase 116 — Auth Hardening SEPs

## Executive Summary

Phase 116 introduces comprehensive auth-hardening additions (covering RFC 9207 `iss` validation, DCR `application_type`, issuer-keyed credential storage, SEP-2351 discovery sequence, CSRF `state` verification, and bounded streaming reads) across the `pmcp` OAuth stack. 

The overall plan architecture is **exceptionally thorough, highly resilient against semver breakage, and strictly security-focused**. The decision to isolate pure validation logic into an ungated, wasm-clean module (`src/shared/oauth_validation.rs`) while adding CI target fences is a standout architectural strength.

---

## Key Strengths & Architectural Highlights

1. **Semver-Safe Extension Strategy**
   - **Struct & Enum Constraints**: Public structs (`DcrRequest`, `OidcDiscoveryMetadata`, `OAuthConfig`) and `Error` are pub-field/constructible without `#[non_exhaustive]`. The plans avoid breaking semver-minor by utilizing inherent accessors over `serde(flatten) extra` maps (`application_type`), marker constants riding on existing `Error::Protocol` variants (`ISS_MISMATCH_MARKER`), and dedicated sibling types (`AuthorizationServerExtras`).
   - **programmatic Discriminators**: Downstream consumers can check `err.is_iss_mismatch()` or `err.is_reauth_required()` cleanly without relying on brittle message substring matching.

2. **Wasm Cleanliness & Platform Seam Decoupling**
   - Pure primitives (`validate_authorization_response`, `discovery_url_candidates`, `derive_application_type`, `CredentialStore` trait) are placed outside the `oauth` feature gate in ungated shared modules (`src/shared/`).
   - **CI Protection**: Plan `116-05` adds an explicit `wasm32-unknown-unknown` CI target fence in the `gate` aggregate job to ensure future contributors cannot silently introduce native dependencies (`reqwest`, `webbrowser`, `dirs`) to the pure tier.

3. **Defensive Security & Information Disclosure Controls**
   - **Order of Evaluation**: `validate_authorization_response` evaluates `state` and `iss` *before* handling AS-returned `error` or `error_description`. This prevents hostile IdPs from using error responses as secret disclosure vectors during invalid/spoofed callbacks.
   - **Anchor Validation**: `issuer_matches_metadata` validates that the document's `issuer` matches the URL seed prior to returning metadata, closing Pitfall 1 (where `iss` comparison would otherwise anchor on attacker-controlled discovery data).
   - **Token Redaction**: `StoredCredentials` manually implements `Debug` with SHA-256 prefixing/redaction, preventing accidental bearer/refresh token exposure in log streams.
   - **Bounded Reads**: Replaces all whole-body `reqwest` needles (`.json()`, `.text()`) with `collect_reqwest_body_within_cap` (streaming 1 MiB cap check) across all auth files (closing D-113-V).

---

## Completeness & Traceability Analysis

| Requirement / SEP | Primary Plan | Implementation Mechanism | Verification Strategy |
| :--- | :--- | :--- | :--- |
| **AUTH-01** (RFC 9207 `iss` Validation) | `116-02`, `116-06`, `116-09` | 4-row spec decision table, ungated `validate_authorization_response`, anchor validation in `fetch_discovery` | Mockito lying-issuer tests, `oauth_iss_validation` binary, fuzz target |
| **AUTH-02** (DCR `application_type`) | `116-03`, `116-04`, `116-10` | Inherent accessors over `extra` map, `derive_application_type` with mixed-vector erroring | Wire-shape tests, collision order tests, `oauth_application_type` binary |
| **AUTH-03** (Issuer-Keyed Credential Storage) | `116-05`, `116-11`, `116-12` | `CredentialStore` trait keyed by `(issuer, account)`, `FileCredentialStore` with atomic 0o600 writes & schema 1→2 migration | Cross-AS lookup isolation tests, schema 1 migration unit tests |
| **D-12** (CSRF `state` Validation) | `116-02`, `116-09` | `state` bound to request record, validated prior to code redemption, non-disclosing error message | CSRF mismatch tests, non-disclosure assertion |
| **D-13 / SEP-2351** (Discovery Path Ordering) | `116-04`, `116-06`, `116-07` | `discovery_url_candidates` returning spec-ordered candidate probe sequence | Probe-order hit-count tests via `expect(0)`, Entra ID fallback test |
| **D-14 / SEP-2207** (Refresh Token Bugs) | `116-12` | Preserving stored refresh token on `None` responses, sourcing DCR `client_id` from store, sending scopes | Headless `RefreshOnly` unit and integration tests |
| **D-15** (Bounded Reads / D-113-V) | `116-06`, `116-07`, `116-12`, `116-14` | Bounded body collection via `collect_reqwest_body_within_cap`, widening `v2_bounded_reads_tripwire.rs` scope | `v2_bounded_reads_tripwire` nextest execution |

---

## Risk Analysis & Technical Pitfalls

1. **Discovery Probe Latency for Path-Bearing Issuers (e.g., Microsoft Entra ID)**
   - **Risk**: For multi-tenant issuers like `https://login.microsoftonline.com/common/v2.0`, candidates 1 and 2 (`oauth-authorization-server` and root `openid-configuration`) will return 404 before candidate 3 (appended `openid-configuration`) succeeds.
   - **Impact**: Initial discovery incurs 2 extra HTTP round-trips for path-bearing IdPs.
   - **Mitigation in Plan**: `cognito.rs` preserves its TTL cache so short-circuiting prevents repeated probes. `generic_oidc.rs` and `client/auth.rs` execute candidate probing in sequence.
   - **Recommendation**: Consider caching the successful candidate index per host/issuer in `AuthorizationServerExtras` or client state to avoid repeating 404 probes on cache miss or client re-instantiation.

2. **Feature-Flag Isolation in Verification Commands**
   - **Risk**: Standard `cargo test` or `make quality-gate` (using `--features full`) does **not** include the `oauth` feature, causing test selectors using `-E 'test(...)'` to quietly select 0 tests and report success.
   - **Mitigation in Plan**: Plan `116-01` explicitly records the A/B baseline between `--features full` (0 tests) and `--features full,oauth` (5+ tests). All per-task verification blocks mandate `--features full,oauth` and specify explicit `-E 'binary(...)'` filters.

3. **Schema 1→2 Migration Token Loss for Unkeyable Legacy Entries**
   - **Risk**: Legacy core `~/.pmcp/oauth-tokens.json` (and schema-1 entries lacking an `issuer` field) cannot be safely migrated because guessing the issuer violates SEP-2352.
   - **Mitigation in Plan**: Unkeyable entries are dropped with a explicit `tracing::warn!` and reported in the migration stats rather than guessing. Users with legacy single-token files will experience a one-time forced re-authentication. This is a sound security tradeoff.

---

## Final Recommendation

The plan set for Phase 116 is **APPROVED** and ready for execution. It exhibits exceptional rigor, strong adherence to security standards, full semver protection, and comprehensive test coverage across unit, property, integration, fuzzing, and CI fencing dimensions.

---

## Codex Review

## Summary

The plans show unusually strong security reasoning, semver awareness, test discipline, and traceability. The pure validation tier, discovery-anchor validation, bounded response handling, and negative-control tests are particularly well designed. However, several structural contradictions make the phase unsafe or impossible to execute as written—most importantly the credential key model, insufficient store API, callback validation timing, refresh-scope handling, and booking AUTH-03 despite a deferred SEP. These should be resolved before implementation begins.

## Strengths

- The `state → iss → authorization error → code` validation order is security-conscious and correctly prevents acting on attacker-controlled error details before issuer validation.
- Requiring `expect(0)` token-endpoint mocks provides strong evidence that rejected codes are never redeemed.
- Validating discovery metadata’s issuer before using it as the RFC 9207 anchor closes an important trust-chain gap that many implementations miss.
- The plans consistently avoid semver-breaking public fields and enum variants through private fields, inherent methods, sibling types, flattened metadata, and marker-based errors.
- Keeping validation and storage traits transport-free and wasm-clean is a good platform-oriented design.
- Ordered discovery candidates preserve compatibility with appended-form-only providers while adding RFC 8414 behavior.
- Streaming, two-stage body limits are materially stronger than post-allocation size checks.
- Verification distinguishes `full` from `full,oauth`, requires non-zero test selection, includes PMAT complexity checks, and uses observed negative controls.
- The wave dependencies are generally coherent and isolate overlapping source files well.

## Concerns

- **HIGH — The credential key is not sufficiently scoped.** `(issuer, account)` collapses credentials for multiple MCP servers using the same authorization server and account. Different servers may have different DCR registrations, client IDs, scopes, audiences, or tokens. Migration can overwrite one entry with another, and logging out of one server can delete credentials used by another. Deferring RFC 8707 makes this more dangerous because resource audience is not otherwise bound.

- **HIGH — `CredentialStore` cannot support Plan 116-13.** Its five methods provide no enumeration, clear-all operation, server-key deletion, or migration-report access. Consequently `auth status`, `auth logout --all`, accurate deletion counts, and reporting migrated/dropped records cannot be implemented as thin wrappers over the declared trait.

- **HIGH — Callback response timing is internally contradictory.** Plan 116-09 has the listener send the raw query and serve browser HTML before the parent validates it. That can display success for a callback later rejected for `state` or `iss`, while the required failure-HTML behavior cannot be selected correctly. Validation must occur before the HTTP response is committed.

- **HIGH — AUTH-01 does not meet its stated compatibility requirement.** `IssPresence::Optional` still rejects a present but mismatched `iss`; therefore “lenient” cannot restore legacy behavior. A v1 deployment behind a rewriting proxy, or an imperfect v1 authorization server, can break despite the claim that the change “cannot break” existing deployments. Either the requirement must be formally amended or target-era/legacy policy must be plumbed into the pre-connection OAuth flow.

- **HIGH — AUTH-03 appears incomplete but is scheduled for `[x]`.** The phase describes six SEPs and explicitly defers SEP-2350, one of the three remaining clarifications covered by AUTH-03’s current wording. Listing that deferral as a limitation does not satisfy the requirement. Amend AUTH-03’s scope or leave it partial.

- **HIGH — `offline_access` is handled at the wrong protocol stages.** Adding it to DCR client metadata does not necessarily request it during authorization. Plan 116-12 then proposes adding advertised `offline_access` to refresh requests even if it was never granted, which can violate refresh-scope rules. It should be requested in the authorization request when policy and advertised support allow it, recorded only if granted, and never newly introduced during refresh.

- **HIGH — Discovery fallback needs an error-classification matrix.** Falling through to later candidates on “any” failure can turn issuer mismatch, malformed metadata, oversized bodies, or other security failures into downgrade/fallback events. Fallback should be limited to explicitly eligible discovery failures; anchor mismatch and malformed security metadata should be terminal.

- **HIGH — The contract-first plan conflicts with repository policy.** Plan 116-01 explicitly concludes that no contract should be authored if none exists, while project instructions require every feature and bug fix to write or update a contract before implementation. The plans also do not explicitly incorporate the mandatory PMAT quality-proxy write workflow.

- **HIGH — The final booking gate cannot complete as written.** Plan 116-15 says to stop if any gate is red, while also asserting that `make doc-check` remains red with 28 errors and that the phase will not clear them. Either the prerequisite doc failures must be resolved, or the acceptance policy must explicitly distinguish an accepted baseline delta from a required green gate. It cannot claim both “every gate green” and “doc-check remains red.”

- **MEDIUM — File-store concurrency is underspecified.** Atomic rename prevents torn files but not lost updates. Concurrent helpers or processes can read the same snapshot and overwrite each other’s credentials or issuer mappings. Credential save and issuer-record updates are also separate operations and can leave inconsistent state.

- **MEDIUM — Several hostile-input cases fail open or remain ambiguous.** Duplicate callback parameters are accepted using “first wins”; a malformed non-boolean RFC 9207 discovery flag likely becomes `None`; invalid environment values silently fall through; and the loopback request line has no stated size cap. Duplicate security parameters should normally be rejected, malformed security metadata should not downgrade strictness, and callback parsing should be bounded.

- **MEDIUM — Discovery URL validation is too permissive.** Parsing as an absolute `url::Url` is not enough. The plan should define allowed schemes, userinfo, fragments, queries, loopback HTTP exceptions, redirect behavior, and whether a cross-origin HTTP redirect is accepted.

- **MEDIUM — DCR override and echo observability lack an implementation path.** The flow always derives and writes `application_type`, so no caller-visible override is described. Likewise, requiring the server-registered value to be observable may tempt adding a field to another public constructible result type, creating the semver break the plans otherwise avoid.

- **MEDIUM — The loopback tests need an explicit test seam.** Tests must capture the generated state, deliver callbacks, prevent real browser launches, and verify listener behavior. No browser-launch/callback transport abstraction is planned. Process-global environment-variable tests also need serialization to avoid races under parallel test execution.

- **MEDIUM — The tripwire anti-vacuity control is logically reversed.** Removing an item from `REQUIRED_FILES` weakens the guard and is expected to pass silently. The meaningful control is to remove a path from `EXTRA_SCOPE` while retaining its full path in the required set. Base-name-only checks are also ambiguous for `auth.rs`.

- **MEDIUM — Some verification commands can mask failures.** Commands using `;` or piping `cargo ... | tail` without pipefail can report success after the important command fails. Non-zero nextest counts should be parsed and asserted, not inferred from `tail`.

- **MEDIUM — Version changes omit `Cargo.lock`.** Updating workspace package versions and running Cargo normally changes the tracked lockfile. Plan 116-13 does not list it among modified files.

- **LOW — The phase is very broad.** It combines callback security, discovery, credential persistence, CLI migration, refresh semantics, body caps, CI changes, versioning, and requirement booking. Ten serial waves increase integration and review risk despite the careful decomposition.

## Suggestions

- Redesign the credential key as at least `(issuer, account, server/resource scope)`, with the issuer remaining mandatory. Add collision tests for two servers sharing one issuer and account.
- Extend the storage contract, or add a separate administrative trait, for listing, clearing, deleting by server, atomic credential-plus-issuer updates, and retrieving migration reports.
- Validate the callback before sending browser HTML. A listener helper can call the same pure validator and send `Result<code>` to the parent, or use a two-way channel so the parent selects success versus failure before the response is written.
- Define an explicit discovery outcome matrix: fallback, retry, or terminal rejection for 404, other 4xx, 5xx, transport errors, invalid JSON, issuer mismatch, oversized bodies, and malformed security metadata.
- Correct the scope lifecycle: request `offline_access` during authorization when appropriate, store the granted scope from token responses, and refresh with only the granted scope or no scope.
- Resolve AUTH-01 and AUTH-03 wording before implementation. Requirement booking should describe behavior already approved by the requirements, not retroactively redefine completion.
- Add the required OAuth contract updates as Wave 1 deliverables and specify PMAT quality-proxy use for every source edit.
- Introduce deterministic browser/callback and environment-policy seams before writing integration tests.
- Make `REQUIRED_FILES` use full relative paths and run the anti-vacuity control by removing scope while retaining the requirement.
- Replace failure-masking shell pipelines with `&&`, `set -o pipefail`, `tee`, or explicit status capture. Include `Cargo.lock` in the version-bump plan.
- Add fuzz/property coverage for malformed credential files, DCR metadata, and body-cap boundaries; the current fuzz plan does not cover all new AUTH-02/AUTH-03 features required by the repository’s ALWAYS policy.
- Consider splitting credential-store and cargo-pmcp migration into a separate phase after the RFC 9207/DCR correctness work is green.

## Risk Assessment

**Overall risk: HIGH.** The plans demonstrate excellent local rigor, but the unresolved credential-collision model, missing store operations, callback timing contradiction, incorrect refresh-scope behavior, compatibility mismatch, and impossible final-gate conditions affect core correctness and requirement completion. Addressing those design issues before execution would substantially reduce the risk; test volume alone cannot compensate for them.

---

## Consensus Summary

### Agreed Strengths

Both reviewers independently praised the same five design choices:

- **Validation ordering** — `state` → `iss` → AS-returned `error` → code redemption. Both
  identified this as deliberately preventing a hostile IdP from using `error_description` as a
  disclosure vector before the issuer is trusted.
- **Discovery anchor validation (Pitfall 1)** — validating the document's `issuer` against the
  issuer used to build the URL *before* the metadata escapes `fetch_discovery`. Both noted that
  without it AUTH-01's entire comparison is anchored on attacker-chosen data.
- **Semver-safe extension strategy** — inherent accessors over `#[serde(flatten)] extra`, marker
  consts on existing `Error::Protocol`, the `AuthorizationServerExtras` sibling type. Both
  confirmed these avoid `constructible_struct_adds_field` MAJOR breaks on a published crate.
- **Wasm-clean pure tier** — `src/shared/oauth_validation.rs` ungated and transport-free, with a
  `wasm32-unknown-unknown` CI fence in the `gate` job.
- **Bounded streaming reads** — two-stage streaming caps judged materially stronger than
  post-allocation size checks.

Both also credited the verification discipline: the `full` vs `full,oauth` A/B baseline,
`binary(...)` selectors, non-zero count assertions, and observed negative controls.

### Agreed Concerns

Only one concern appears in both reviews, and from different angles:

- **Discovery fallback behavior** — Gemini raised it as *latency* (2 extra 404 round-trips for
  path-bearing issuers like Entra ID, recommending the successful candidate index be cached).
  Codex raised it as *security* (HIGH: falling through on "any" failure turns issuer mismatch,
  malformed metadata and oversized bodies into silent downgrade events; fallback must be limited
  to explicitly eligible failures, with anchor mismatch terminal). Both point at the same
  under-specified fallback logic. **The union of the two is an explicit discovery outcome matrix
  — per-status-class, with a caching decision — which neither review alone fully specifies.**

### Divergent Views

The reviewers disagree on the phase's readiness, and the disagreement is not cosmetic.

| | Gemini | Codex |
|---|---|---|
| Verdict | **APPROVED** — "ready for execution" | **HIGH risk** — "resolve before implementation begins" |
| HIGH concerns | 0 | 9 |
| Framing | Audited what the plans *say they do*, and found the stated mechanisms sound | Audited whether the plans are *internally consistent and executable*, and found contradictions |

Codex raised nine HIGH findings Gemini did not surface at all. The most consequential:

1. **Credential key under-scoped** — `(issuer, account)` collapses credentials for multiple MCP
   servers sharing one authorization server and account. Different servers may hold different DCR
   registrations, client IDs and scopes; migration can overwrite one with another, and `logout` on
   one server can delete another's credentials. Deferring RFC 8707 (owner decision, commit
   `b2bf9157`) removes the audience binding that would otherwise mitigate this.
2. **`CredentialStore` cannot support Plan 116-13** — the five declared methods offer no
   enumeration, clear-all, delete-by-server, or migration-report access, so `auth status`,
   `auth logout --all` and accurate deletion counts cannot be thin wrappers over the trait.
3. **Callback timing contradiction** — 116-09 has the listener serve browser HTML before the
   parent validates, so a callback later rejected for `state`/`iss` can already have displayed
   success, and the required failure-HTML branch is unselectable.
4. **AUTH-01 lenient mode does not restore legacy behavior** — `IssPresence::Optional` still
   rejects a *present but mismatched* `iss`, so the "cannot break existing deployments" claim
   fails for a v1 deployment behind a rewriting proxy.
5. **AUTH-03 booked `[x]` while SEP-2350 is deferred** — one of the three clarifications inside
   AUTH-03's own wording. Listing it as a limitation does not satisfy the requirement.
6. **116-15's gate policy is self-contradictory** — it says stop if any gate is red while also
   asserting `make doc-check` stays red at 28 errors. It cannot claim both.
7. **`offline_access` handled at the wrong protocol stage** — belongs in the authorization
   request when advertised, recorded only if granted, never newly introduced at refresh.
8. **Contract-first conflict** — 116-01 concludes no contract need be authored, which contradicts
   CLAUDE.md's contract-first mandate; the PMAT quality-proxy write workflow is also unaddressed.
9. **Tripwire anti-vacuity control is logically reversed** — removing from `REQUIRED_FILES`
   weakens the guard and passes silently; the meaningful control removes from `EXTRA_SCOPE`.

**Assessment of the divergence:** these are largely *cross-plan consistency* failures — a trait
declared in 116-05 against consumers written in 116-13, a listener in 116-09 against its own
error-HTML requirement, a requirement booked in 116-15 against a deferral made in `b2bf9157`.
A reviewer reading each plan on its own merits would not see them. Codex's findings are the more
actionable of the two reviews and should drive the replan; Gemini's latency observation on
discovery caching should be folded into the same outcome-matrix work.

Items 1, 2, 4, 5 and 6 change requirement wording or public API shape and cannot be resolved by
the planner alone — they need owner decisions.
