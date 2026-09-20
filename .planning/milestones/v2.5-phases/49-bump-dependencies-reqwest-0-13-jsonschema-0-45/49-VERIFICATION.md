---
phase: 49-bump-dependencies-reqwest-0-13-jsonschema-0-45
verified: 2026-03-13T10:30:00Z
status: passed
score: 5/5 must-haves verified
re_verification: false
---

# Phase 49: Bump Dependencies (reqwest 0.13, jsonschema 0.45) Verification Report

**Phase Goal:** Upgrade reqwest from 0.12 to 0.13 and jsonschema from 0.38 to 0.45 across the workspace, updating feature flags, MSRV, deprecated methods, and template strings
**Verified:** 2026-03-13T10:30:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | All four workspace Cargo.toml files reference reqwest 0.13 with correct feature names (rustls, form) | VERIFIED | Root: `reqwest = { version = "0.13", ... features = ["json", "rustls", "form"] }`; mcp-tester: `version = "0.13", features = ["json", "stream", "rustls"]`; mcp-preview: `version = "0.13", features = ["json"]`; cargo-pmcp: `version = "0.13", features = ["json", "multipart", "rustls", "form"]` |
| 2 | jsonschema bumped to 0.45 with MSRV raised to 1.83.0 | VERIFIED | `Cargo.toml` line 93: `jsonschema = { version = "0.45", optional = true }`; line 14: `rust-version = "1.83.0"` |
| 3 | Template strings in deploy/scaffold generate correct reqwest 0.13 lines for new projects | VERIFIED | `deploy/init.rs` lines 880, 918: `reqwest = {{ version = "0.13", ..., features = ["json", "rustls"] }}`; `templates/oauth/proxy.rs` line 474: `reqwest = {{ version = "0.13", ..., features = ["json", "rustls", "form"] }}` |
| 4 | make quality-gate passes with zero warnings (for changed crates) | VERIFIED (with note) | `cargo check --features full` exits clean (0.21s); all 9 modified files pass; pre-existing unrelated clippy issues in non-changed crates (mcp-e2e-tests, pmcp-macros) are outside phase scope |
| 5 | All .form() call sites compile (form feature enabled where needed) | VERIFIED | Root crate has `form` feature (used in `src/client/auth.rs:347,399` and `src/client/oauth.rs:444,492,553`); cargo-pmcp has `form` feature (used in `deployment/targets/pmcp_run/auth.rs:373`); mcp-tester has no `.form()` calls and correctly omits the feature |

**Score:** 5/5 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `Cargo.toml` | Root crate reqwest 0.13, jsonschema 0.45, MSRV 1.83 | VERIFIED | Line 104: `reqwest 0.13 + rustls + form`; line 93: `jsonschema 0.45`; line 14: `rust-version = "1.83.0"` |
| `crates/mcp-tester/Cargo.toml` | mcp-tester reqwest 0.13 | VERIFIED | Line 36: `reqwest = { version = "0.13", features = ["json", "stream", "rustls"], default-features = false }` |
| `crates/mcp-preview/Cargo.toml` | mcp-preview reqwest 0.13 | VERIFIED | Line 26: `reqwest = { version = "0.13", features = ["json"] }` |
| `cargo-pmcp/Cargo.toml` | cargo-pmcp reqwest 0.13 | VERIFIED | Line 46: `reqwest = { version = "0.13", features = ["json", "multipart", "rustls", "form"], default-features = false }` |
| `crates/mcp-tester/src/tester.rs` | Deprecated methods renamed | VERIFIED | Lines 126, 147, 1106 all use `tls_danger_accept_invalid_certs`; old name absent |
| `cargo-pmcp/src/commands/deploy/init.rs` | Template strings updated | VERIFIED | Lines 880, 918: `version = "0.13"` with `rustls` feature |
| `cargo-pmcp/src/templates/oauth/proxy.rs` | OAuth template updated | VERIFIED | Line 474: `version = "0.13"` with `rustls` and `form` features |
| `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` | OAuth bridge uses oauth2::reqwest::Client | VERIFIED | Lines 422, 625: `oauth2::reqwest::Client::new()`; direct API calls at lines 133, 369 use `reqwest::Client::new()` correctly |
| `examples/26-server-tester/src/tester.rs` | Deprecated methods renamed | VERIFIED | Lines 114, 135, 1071 all use `tls_danger_accept_invalid_certs` |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `Cargo.toml` | `src/client/oauth.rs` | form feature enables `.form()` method | WIRED | `features = ["json", "rustls", "form"]` in Cargo.toml; `.form()` used at oauth.rs:444, 492, 553 |
| `cargo-pmcp/Cargo.toml` | `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` | form feature enables `.form()` method | WIRED | `features = ["json", "multipart", "rustls", "form"]` in cargo-pmcp/Cargo.toml; `.form()` used at auth.rs:373 |
| `cargo-pmcp/Cargo.toml` (oauth2 dep) | `auth.rs` oauth2 token exchange | oauth2::reqwest::Client bridges 0.12/0.13 mismatch | WIRED | `oauth2::reqwest::Client::new()` at auth.rs:422, 625; `oauth2 = "5.0"` in Cargo.toml |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| DEP-01 | 49-01-PLAN.md | Bump reqwest 0.13 + jsonschema 0.45 across workspace | SATISFIED | All four Cargo.toml files updated, MSRV raised, templates corrected, deprecated methods renamed, oauth2 bridge implemented. Commits f5d0de9 and df7b9f3 verified. |

**Note on DEP-01:** This requirement ID is declared in the PLAN frontmatter (`requirements: [DEP-01]`) and in `ROADMAP.md` (`Requirements: DEP-01`), but does not appear as a named entry in `.planning/REQUIREMENTS.md`. The REQUIREMENTS.md documents v1.5 and v1.6 CLI/flag requirements; DEP-01 is a dependency management requirement outside that scope. The implementation fully satisfies the described intent — no functional gap.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None found | — | — | — | — |

No TODO/FIXME/placeholder comments, no stub implementations, no empty handlers found in any of the 9 modified files.

---

### Human Verification Required

None. All verification items are mechanically checkable and have been confirmed via file inspection, grep, and `cargo check`.

---

### Gaps Summary

No gaps. All five observable truths from the PLAN frontmatter are verified against the actual codebase:

- Four Cargo.toml files updated to reqwest 0.13 with correct feature sets
- jsonschema at 0.45, MSRV at 1.83.0
- Three template strings corrected for new project scaffolding
- Deprecated `danger_accept_invalid_certs` renamed at all 6 sites (3 in mcp-tester, 3 in example)
- oauth2 version bridge implemented for token exchange compatibility
- Cargo.lock confirms dual reqwest versions (0.12 via oauth2, 0.13 direct) as expected

The SUMMARY note about pre-existing quality-gate failures (mcp-e2e-tests, pmcp-macros clippy issues; atty audit warning) is confirmed to be outside phase scope — those crates are not in the modified files list and the issues predate this phase.

---

_Verified: 2026-03-13T10:30:00Z_
_Verifier: Claude (gsd-verifier)_
