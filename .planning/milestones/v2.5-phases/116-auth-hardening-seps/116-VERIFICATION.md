---
phase: 116-auth-hardening-seps
verified: 2026-08-06T00:00:00Z
status: passed
score: 3/3 must-haves verified
overrides_applied: 0
---

# Phase 116: Auth Hardening SEPs Verification Report

**Phase Goal:** The v2 auth-hardening SEPs land as hand-rolled source changes to the existing OAuth
stack — strict on v2, lenient on v1 — so existing deployments (Lambda `oauth_passthrough`, the
Graph/M365 example, documented proxy exceptions) keep working. Fully independent — parallelizes with
Phases 113-115.
**Verified:** 2026-08-06 (re-derived independently from HEAD `a69deebd`, not from SUMMARY narration)
**Status:** passed
**Re-verification:** No — initial verification

## Method

This verification does not re-run the full `make quality-gate` battery (116-15 already did, at this
exact HEAD, with `make quality-gate` re-run green after the final commit and no source drift since —
confirmed via `git log b0b92cfd..HEAD -- src/ cargo-pmcp/src/ contracts/binding.yaml
.planning/REQUIREMENTS.md` returning nothing). Instead it independently re-derives the specific,
falsifiable claims the SUMMARY makes: existence and content of the artifacts, function signatures
cited in `contracts/binding.yaml`, the file lists and error counts cited as gate evidence, and a
targeted re-run of the highest-load-bearing test binaries under the host's documented `SSL_CERT_FILE`
workaround. Every number below marked "RE-RUN" was executed fresh in this session, not read from a
log.

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | OAuth callback validates RFC 9207 `iss` — strict whenever the AS advertises the flag or emits `iss`, present-but-mismatched rejected on every era, v1 leniency narrowed to tolerate only an absent `iss` (AUTH-01, amended text) | VERIFIED | `validate_authorization_response` and `iss_presence_from` exist at `src/shared/oauth_validation.rs:334,530` exactly matching `contracts/binding.yaml`'s cited signatures. `tests/oauth_iss_validation.rs` has 27 `#[test]` functions (grep-counted), `tests/oauth_iss_integration.rs` has 13 — both match the booked counts. `git diff --name-only b2bf9157..HEAD -- src/` independently reproduces the exact 12-file list `deferred-items.md` cites for the doc-check attribution argument, confirming the file-level claims are not fabricated. |
| 2 | Dynamic client registration sends and accepts `application_type` (AUTH-02) | VERIFIED | `DCR_APPLICATION_TYPE_KEY` / `derive_application_type` exist (`src/shared/oauth_validation.rs:1104`); `application_type` rides the pre-existing `#[serde(flatten)] extra` carrier on `DcrRequest`/`DcrResponse`, not a new field — confirmed no new field was added by grepping the struct definitions in `src/server/auth/provider.rs`, consistent with the semver-additive claim. |
| 3 | Credential storage keyed by `(issuer, account, server)` plus SEP-2351 + SEP-2207, SEP-2350 explicitly out of scope, no v1 breakage, no `oauth2`/`openidconnect` added to core (AUTH-03, amended text) | VERIFIED | `CredentialKey::new<I,A,S>` confirmed 3-parameter at `src/shared/credential_store.rs:152`. Dependency fence RE-RUN (see below) reproduces exit 1 (no hits) on all four grep commands. `cargo-pmcp`'s `oauth2::` usage confirmed confined to `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` (zero hits under `cargo-pmcp/src/commands/`). Root `pmcp` `Cargo.toml` has no `oauth2`/`openidconnect` line. `TokenCacheV1` no longer exists in `cargo-pmcp/src/` (only a stale doc-comment reference to the pattern it replaced). |

**Score:** 3/3 truths verified.

### Verify-These-Specifically Checklist (from the launch instructions)

| # | Item | Verdict | Evidence |
|---|------|---------|----------|
| 1 | AUTH-03's booking quotes `D-116-PRM` rather than overclaiming | VERIFIED | `.planning/REQUIREMENTS.md:799-819` quotes `D-116-PRM` from `deferred-items.md:2007-2044` **verbatim** (compared both blocks side by side — byte-identical prose). The quote states plainly: "the D-116-R1 collision is not CONSTRUCTIBLE through the live flow... `116-11`'s collision test therefore SEEDS the second server's entry" — this is an honest disclosure, not a claim of end-to-end coverage. |
| 2 | The credential-store seam is real, single-path, no reach-around | VERIFIED | `CredentialStore`/`CredentialStoreAdmin` traits at `src/shared/credential_store.rs:807,877`; `FileCredentialStore` at `src/shared/credential_file.rs:189`. `cargo-pmcp/src/commands/auth_cmd/*.rs` imports only `pmcp::shared::credential_store::{CredentialStore, CredentialStoreAdmin, ...}` and `pmcp::FileCredentialStore` — zero `std::fs::`/`tokio::fs::`/`File::open`/`File::create`/direct-serde calls anywhere in `cargo-pmcp/src/commands/auth_cmd/`. No parallel `TokenCacheV1` implementation exists any more (grep finds only a comment referencing the retired pattern). |
| 3 | Gate-scope hole (143 vs 180 of 323) is honestly derived | VERIFIED | Arithmetic re-checked independently: the 13 "invisible to gate" rows in `deferred-items.md`'s A2 table sum to 117 (core) + 26 (cargo-pmcp) = 143; the 7 "covered" rows sum to 180; 143+180=323. RE-RUN: `binary(oauth_credential_store)+binary(oauth_store_wiring)+binary(v2_bounded_reads_tripwire)` → **85 tests run: 85 passed** (54+18+13, matches table exactly). RE-RUN: cargo-pmcp `binary(auth_integration) + (binary(cargo_pmcp) and test(auth_cmd))` → **26 tests run: 26 passed (1 leaky)** (20+6, matches table exactly, including the `LEAK` marker on one test — still a pass, not silently dropped). |
| 4 | Both environmental deviations (`SSL_CERT_FILE`, rust-analyzer) declared everywhere gate results are recorded | VERIFIED | `deferred-items.md:72-88` states both deviations in a dedicated subsection immediately before any gate number, explicitly: "Two environmental deviations apply to every number below... declared here so no reader mistakes these for clean-room numbers." `116-15-SUMMARY.md` Issues #2/#3 restate both. `STATE.md` does not restate them per-line but points to the same plan. No gate table anywhere presents a number without this preceding disclosure. |
| 5 | `LIM-116-10` and SEP-2350 recorded as deferrals with named/UNASSIGNED owners, not silently dropped or misfiled as AUTH-03 limitations | VERIFIED | `LIM-116-10` (`deferred-items.md:1112-1134`): Owner **UNASSIGNED**, Status "OPEN, REASSIGNED from `116-15`" — explicitly NOT listed among AUTH-03's "three limitations that ARE in scope" (`REQUIREMENTS.md:892-907`, which lists only AS-change detection mechanism, schema-1 dropped-issuer entries, and the released cargo-pmcp 0.18.0 hard-error — `LIM-116-10` is absent from that list, correctly). `DEF-116-03`/SEP-2350 (`deferred-items.md:765-779`): Owner **UNASSIGNED — needs its own phase**, Status OPEN, and `REQUIREMENTS.md:887-890` states explicitly "SEP-2350 is NOT listed as a limitation of this requirement... recorded as a DEFERRAL in the phase register instead." |

### Independent Re-Runs (executed this session, not read from a log)

| Check | Command | Result | Matches claim? |
|---|---|---|---|
| Credential-store + wiring + tripwire tests | `cargo nextest run --features full,oauth -E 'binary(oauth_credential_store)+binary(oauth_store_wiring)+binary(v2_bounded_reads_tripwire)'` (with `SSL_CERT_FILE`) | `85 tests run: 85 passed, 0 skipped` | Yes — 54+18+13=85 |
| cargo-pmcp auth tests | `cargo nextest run -p cargo-pmcp -E 'binary(auth_integration) + (binary(cargo_pmcp) and test(auth_cmd))'` | `26 tests run: 26 passed (1 leaky), 454 skipped` | Yes — 20+6=26 |
| `make doc-check` | `/usr/bin/make doc-check` | exit 101 (make exit 2), **28** `^error` lines, the `src/error/mod.rs:613` ambiguous-link error present verbatim | Yes — matches B1/B2 anchor and the specific line/text quoted in `deferred-items.md` |
| Changed-file list for B2's non-attribution argument | `git diff --name-only b2bf9157..HEAD -- src/` | Exactly the 12 files `deferred-items.md` lists | Yes |
| Dependency fence (4 commands) | see AUTH-03 clause 5 commands | All 4 reproduce exit 1 / no hits | Yes |
| `make check-todos` | `/usr/bin/make check-todos` | exit 0, "No technical debt comments" | Yes |
| Debt markers in phase-touched files | `grep -n "TBD\|FIXME\|XXX"` across the 12 changed `src/` files + `auth_cmd/*.rs` | 0 hits | Yes — no undisclosed debt markers |
| `wasm32-purity` in `gate`'s `needs:` | `grep -n wasm32-purity .github/workflows/ci.yml` | present at line 404 (job def) and line 443 (`needs:` array) | Yes |
| `status: planned` remaining in `contracts/binding.yaml` | `grep -c '^  status: planned'` | **0** | Yes — all 8 Phase 116 bindings flipped |
| All 8 bound function signatures exist | grep for each `pub fn`/`pub fn new<...>` cited in `contracts/binding.yaml` Phase 116 section | All 6 `oauth_validation.rs` functions + `CredentialKey::new` + `parse_credential_snapshot` found at cited or equivalent lines | Yes |

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `src/shared/oauth_validation.rs` | RFC 9207/8414/SEP-2351 decision tables, ungated | VERIFIED | 1376 lines; `validate_authorization_response`, `iss_presence_from`, `discovery_url_candidates`, `issuer_matches_metadata`, `classify_discovery_failure`, `derive_application_type` all present and public |
| `src/shared/credential_store.rs` | `CredentialKey`, `CredentialStore`/`CredentialStoreAdmin`, migration | VERIFIED | 1080 lines; traits + `InMemoryCredentialStore` + `parse_credential_snapshot` confirmed |
| `src/shared/credential_file.rs` | `FileCredentialStore`, atomic/locked writes | VERIFIED | 690 lines; `FileCredentialStore` struct confirmed; at-rest tests (0600/0700, stale-lock break) present and independently re-run passing as part of the 85-test batch above (transitively, via the wiring tests that call into it) |
| `src/client/oauth.rs` | loopback listener, validate-before-respond, refresh path | VERIFIED | 3756 lines; contains `registration_rejected`, browser-launcher seam, no unresolved stubs found in anti-pattern scan |
| `cargo-pmcp/src/commands/auth_cmd/` | thin wrapper over the trait, no parallel store | VERIFIED | 7 files; imports only `CredentialStore`/`CredentialStoreAdmin`/`FileCredentialStore`; zero direct filesystem calls |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `cargo-pmcp/src/commands/auth_cmd/*.rs` | `pmcp::shared::credential_store` | trait imports (`CredentialStore`, `CredentialStoreAdmin`) | WIRED | No parallel file I/O found; single path confirmed by absence of `std::fs`/`tokio::fs`/`File::open` in the command modules |
| `src/client/oauth.rs` | `src/shared/oauth_validation.rs` | `validate_authorization_response` called inside the loopback listener before response is committed | WIRED | Booking text cites `binary(oauth_iss_integration)::an_error_description_behind_a_wrong_iss_reaches_neither_the_error_nor_the_browser`, independently confirmed present (`oauth_iss_integration.rs` has 13 tests, matching count) |
| `src/server/auth/provider.rs` (`DcrRequest`/`DcrResponse`) | wire body | `#[serde(flatten)] extra` carrier, not a new field | WIRED | Semver-additive claim consistent with the flatten-carrier pattern found in the struct; `oauth_dcr_integration.rs` asserts on the wire body per its own test names |
| Two-servers-one-AS collision defense | `CredentialKey`'s 3-part shape | store + trait + helper, NOT the live discovery flow | PARTIAL, disclosed | This is the one link the phase itself documents as incomplete (`D-116-PRM`) — the key shape is wired and proven at the store; the live-flow scenario that would exercise it end-to-end cannot occur until RFC 9728 lands. Correctly disclosed, not concealed. |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| AUTH-01 | 116-01 (declared), 116-02/06/07/09/15 (delivered) | RFC 9207 `iss` validation, amended text | SATISFIED | `oauth_validation.rs` functions exist; 27/13/12/19 test counts re-derivable; amendment text in `REQUIREMENTS.md` matches `0aebf7f6` |
| AUTH-02 | 116-01/03/10/15 | DCR `application_type` | SATISFIED | wire-body assertion tests present; semver-additive via flatten carrier |
| AUTH-03 | 116-01/04/05/06/07/10/11/12/13/14/16/15 | Credential storage key + SEP-2351/2207, SEP-2350 out of scope | SATISFIED, with disclosed precondition `D-116-PRM` | `CredentialKey::new<I,A,S>`, dependency fence reproduced, `D-116-PRM` quoted verbatim in the booking |

No orphaned requirements: `116-01-PLAN.md` declares `requirements: [AUTH-01, AUTH-02, AUTH-03]`, and `REQUIREMENTS.md`'s Phase 116 section covers exactly those three, matching the ROADMAP's `Requirements: AUTH-01, AUTH-02, AUTH-03`.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| `src/client/oauth.rs` | 377-379 | "deliberately not implemented" (SEP-837 optional retry) | ℹ️ Info | Documented, intentional non-adoption of a spec MAY, consistent with AUTH-02's booking text — not a stub of required functionality |

No `TBD`/`FIXME`/`XXX`/`HACK` found in any of the 12 phase-touched `src/` files or `cargo-pmcp/src/commands/auth_cmd/`. `make check-todos` (the repo's actual gate, scoped to `src/`) independently re-run: exit 0.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Credential store / wiring / bounded-reads test suite executes and passes | `cargo nextest run --features full,oauth -E 'binary(oauth_credential_store)+binary(oauth_store_wiring)+binary(v2_bounded_reads_tripwire)'` | 85/85 passed | PASS |
| cargo-pmcp auth CLI wrapper test suite executes and passes | `cargo nextest run -p cargo-pmcp -E 'binary(auth_integration)+(binary(cargo_pmcp) and test(auth_cmd))'` | 26/26 passed (1 leaky, not failed) | PASS |
| `make doc-check` reproduces the claimed anchor and non-attributed error | `/usr/bin/make doc-check` | 28 errors, `src/error/mod.rs:613` verbatim | PASS |
| `make check-todos` | `/usr/bin/make check-todos` | exit 0 | PASS |

Full `make quality-gate` / `cargo semver-checks` / `make wasm-build` / fuzz campaigns were **not** re-run in this session per the launch instructions (116-15 ran them at this exact HEAD with no source drift since, confirmed via `git log b0b92cfd..HEAD -- src/ cargo-pmcp/src/`). The components re-run above were chosen because they carry the highest evidentiary weight for AUTH-01/02/03 and for the two host-environment deviations, and all reproduced exactly.

### Human Verification Required

None. This phase is entirely backend/library OAuth-flow logic with no UI surface. The one behavior
that would ordinarily require a human (opening a real browser and completing an interactive OAuth
consent screen) is deliberately made testable without a human via the `BrowserLauncher` seam
(`src/client/oauth.rs`, `116-09`) — the interactive path is exercised end-to-end with a mock launcher
and a mock `/token` endpoint (`expect(0)`/`assert_async()` assertions), which this verification
confirmed executes and passes. No visual, real-time, or external-service behavior in this phase
resists programmatic verification.

### Gaps Summary

None found. The one incompletely-closed thread — the `D-116-R1` two-servers-one-authorization-server
collision defense having no live-flow end-to-end coverage until RFC 9728 (`DEF-116-01`) lands — is not
a gap: it is honestly disclosed as a named precondition (`D-116-PRM`), quoted verbatim in the
requirement booking, with the underlying key-shape mechanism itself fully implemented and tested.
`LIM-116-10` (143 tests + 17 clippy diagnostics outside `make quality-gate`/`make lint`'s scope) is a
real, material coverage hole in the project's CI surface, but it is explicitly out of this phase's
scope (its owner, 116-15, cannot touch the `Makefile` per its own task list) and is correctly recorded
as `UNASSIGNED`, not concealed or misattributed. Both should be prioritized as roadmap items but do
not block Phase 116's own goal, which was to land the SEP source changes — which they did, verified
independently rather than taken on the SUMMARY's word.

---

_Verified: 2026-08-06_
_Verifier: Claude (gsd-verifier)_
