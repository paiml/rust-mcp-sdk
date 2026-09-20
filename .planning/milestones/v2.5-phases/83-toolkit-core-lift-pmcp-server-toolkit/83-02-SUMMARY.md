---
phase: 83-toolkit-core-lift-pmcp-server-toolkit
plan: 02
subsystem: toolkit-auth-secrets
tags:
  - toolkit
  - auth
  - secrets
  - lift
  - review-r3
  - review-r5
  - review-r6
requirements:
  - TKIT-02
  - TKIT-03
status: complete
---

# Plan 02 Summary — Lift `auth.rs` + `secrets.rs` with toolkit-owned `SecretValue`

## What Was Built

Plan 02 lifted the auth + secrets layer from external `pmcp-run/built-in/shared/mcp-server-common/` into `pmcp-server-toolkit`, with the three review-driven deltas applied (R3 crate-root re-exports, R5 trybuild compile-fail tests, R6 toolkit-owned `SecretValue`).

### Task 1 — `auth.rs` lift (commit `f6073fb0`)

- New `StaticAuthProvider { expected_token }` in `crates/pmcp-server-toolkit/src/auth.rs` impls `pmcp::server::auth::AuthProvider::validate_request` per 83-PATTERNS.md §3.
- Token comparison uses a local `constant_time_eq` helper (byte-XOR accumulation, no short-circuit) to block timing-side-channel leaks during dev/test use (T-83-02-01 mitigation).
- 7 unit tests + 1 doctest: valid token, mismatched token, missing header, non-Bearer scheme, case-insensitive prefix, and the helper's mismatch/length-disparity cases.
- Shape-divergence note: `mcp-server-common::auth` models OUTBOUND HTTP backend auth (apply auth to outgoing reqwest::HeaderMap), whereas `pmcp::server::auth::AuthProvider` models INBOUND request validation. PATTERNS §3 already specifies the `StaticAuthProvider` shape — this lift follows that authoritative shape (rule 2 deviation).

### Task 2 — `secrets.rs` lift + toolkit-owned `SecretValue` (commit `7a57d7d5`)

Verbatim lift of secrets.rs from `pmcp-run/built-in/shared/mcp-server-common/` with four mechanical deltas (crate-path swap, pmcp dep shape, feature-gate trim per D-14, attribution header), plus the review-driven additions:

**Review R6 — toolkit-owned `SecretValue`:**
- New `SecretValue` newtype wrapping `secrecy::SecretBox<[u8]>` — available unconditionally, NOT dependent on the `code-mode` feature. `cargo build -p pmcp-server-toolkit --no-default-features` succeeds, proving feature-independence.
- `impl From<SecretValue> for pmcp_code_mode::TokenSecret` is gated on `code-mode` for interop with the HMAC token machinery.
- `SecretsProvider::get` returns `Result<SecretValue, ToolkitError>` — never `Result<String>`, `Result<Vec<u8>>`, or `Result<TokenSecret>` (PATTERNS §Anti-Patterns #11).

**Review R5 — REAL trybuild compile-fail tests** (replacing the prior commented-out theatre):
- `tests/compile_fail/token_secret_no_debug.rs` — `println!("{:?}", s)` fails to compile
- `tests/compile_fail/token_secret_no_clone.rs` — `s.clone()` fails to compile
- `tests/compile_fail/token_secret_no_serialize.rs` — `serde_json::to_string(&s)` fails to compile
- `.stderr` baselines committed alongside each `.rs` source
- `tests/trybuild.rs` harness runs all three
- Compile-fail caught a real violation during implementation: `result.unwrap_err()` on `Result<SecretValue, _>` requires `T: Debug`, which `SecretValue` intentionally lacks. Test rewritten to use a manual `match` — review R5 enforcement working as designed.

**Concrete provider implementations:**
- `EnvSecrets` (unconditional, prefix-filtered)
- `OrgSecretsManagerProvider`, `SecretsManagerSecrets`, `SsmSecrets` (all `#[cfg(feature = "aws")]` per D-14)
- `SecretsProviderChain` for ordered fallthrough
- `create_secrets_provider(server_name)` factory honoring `PMCP_SECRETS_PATH` / `PMCP_SSM_PATH` / `PMCP_SERVER_ID`

**Threat-register additions:**
- `ToolkitError::Secret { name, cause }` variant — never carries raw secret bytes, only lookup-key metadata (T-83-02-02).

**Test count:** 8 unit tests in `secrets.rs` (SecretValue Send+Sync, byte exposure, env-var Ok/Err, prefix filter, no-prefix mode, chain fallback, org-path detection) + 1 doctest on `EnvSecrets::new` + 3 trybuild compile-fail assertions.

### Task 3 — Crate-root re-exports per D-15 + /simplify cleanup (commit `131172e6`)

**Review R3 — crate-root re-exports (the headline D-15 DX promise):**
- `pub use pmcp::server::auth::AuthProvider;` at the toolkit crate root (verified pmcp path; NOT `auth::AuthProvider as _`)
- `pub use crate::auth::StaticAuthProvider;`
- `pub use crate::secrets::{EnvSecrets, SecretValue, SecretsProvider, SecretsProviderChain};`
- AWS-feature-gated: `pub use crate::secrets::{OrgSecretsManagerProvider, SecretsManagerSecrets, SsmSecrets};`
- Compile-only `const _ROOT_REEXPORT_SMOKE` asserts each symbol resolves at the crate root.

**/simplify pass corrections:**
- `SecretValue::new` double-copy eliminated (was `Box::from(v.as_slice())` leaving plaintext residue in `v`; now `bytes.into().into_boxed_slice()` — move-only).
- Verbose attribution-header prose trimmed in `auth.rs` and `secrets.rs` to the standard 3-line form (the lift-deltas detail belongs in this SUMMARY, not source).
- `EnvSecrets::full_name`: short-circuit on empty prefix to skip a `format!` allocation per lookup.
- WHAT-narrating SAFETY NOTE comment block deleted — trybuild tests are the actual enforcement.
- `tokio` dev-dep switched from `rt-multi-thread` to `rt` (tests run `--test-threads=1`).

## Quality Gates

- `cargo check -p pmcp-server-toolkit` (default features): ✓
- `cargo check -p pmcp-server-toolkit --no-default-features` (proves R6 feature-independence): ✓
- `cargo check -p pmcp-server-toolkit --tests`: ✓
- `cargo test -p pmcp-server-toolkit --lib`: 16 passed
- `cargo test -p pmcp-server-toolkit --doc`: 4 passed
- `cargo clippy -p pmcp-server-toolkit --all-targets -- -D warnings`: no issues
- `make quality-gate` (workspace-wide): exit 0

## Files Modified

| File | Role |
|------|------|
| `crates/pmcp-server-toolkit/src/auth.rs` | New 200+ LoC — `StaticAuthProvider` + `constant_time_eq` + tests |
| `crates/pmcp-server-toolkit/src/secrets.rs` | New 850+ LoC — `SecretValue`, `SecretsProvider` trait, 5 concrete providers, `SecretsProviderChain`, factory |
| `crates/pmcp-server-toolkit/src/error.rs` | +`Secret { name, cause }` variant + `From<env::VarError>` |
| `crates/pmcp-server-toolkit/src/lib.rs` | Crate-root re-exports for D-15 / R3 + compile-only smoke const |
| `crates/pmcp-server-toolkit/Cargo.toml` | +secrecy (unconditional), +tokio (aws-gated), trimmed dev-dep tokio features |
| `crates/pmcp-server-toolkit/tests/trybuild.rs` | New — harness for compile-fail R5 tests |
| `crates/pmcp-server-toolkit/tests/compile_fail/token_secret_no_{debug,clone,serialize}.rs` + `.stderr` | New — real R5 enforcement |
| `CLAUDE.md` | /simplify: publish order updated with `pmcp-code-mode` + `pmcp-code-mode-derive` |

## Requirement Coverage

- **TKIT-02** — `AuthProvider` exposed via public toolkit API with `StaticAuthProvider` impl ✓
- **TKIT-03** — `SecretsProvider` exposed with concrete `EnvSecrets` (unconditional) + 3 AWS-feature-gated impls ✓

## CONTEXT.md Decisions Honored

- **D-13** — `SecretsProvider::get` returns toolkit-owned `SecretValue`, never raw `String` or `Vec<u8>`.
- **D-14** — AWS providers behind `aws` feature; `secrecy` dep is unconditional per the review R6 stability requirement.
- **D-15** — Crate-root re-exports for the headline imports.
- **D-16** — `code-mode` interop via feature-gated `impl From<SecretValue> for TokenSecret`, not via cross-crate trait extension.

## Review R-IDs Closed

- **R3** — Crate-root re-exports use verified `pmcp::server::auth::AuthProvider` path; no `as _` no-name imports; compile-only smoke const enforces.
- **R5** — Real `trybuild` compile-fail tests replace commented-out negative-trait theatre.
- **R6** — `SecretValue` is toolkit-owned and feature-independent (`--no-default-features` builds clean).

## Deferred (raised by /simplify, not addressed in Plan 02)

These were flagged by the parallel reuse + quality + efficiency review but each is either a larger design decision or out-of-Plan-02 scope:

- Replace hand-rolled `constant_time_eq` with the `subtle` crate (adds dep on a crypto-sensitive primitive — wants user approval).
- Extract a `CachedKvSecrets<F>` helper to dedupe the three near-identical AWS providers (~30 LOC of locking ceremony — better as a follow-up refactor plan).
- Switch AWS cache value type from `String` to `SecretValue` (removes the intermediate plaintext-String window; needs API decision on `Arc<SecretValue>` for the trait return).
- `StaticAuthProvider.expected_token: String` → `SecretValue` (public-API change — should land before crates.io publish).
- Re-evaluate `SecretValue` ↔ `pmcp_code_mode::TokenSecret` deduplication (contradicts review R6's "feature-independent" requirement; would need a `pmcp-secret` leaf-crate split).

## Commits

| SHA | Subject |
|-----|---------|
| `f6073fb0` | feat(83-02): lift auth.rs as StaticAuthProvider impling pmcp AuthProvider |
| `7a57d7d5` | feat(83-02): lift secrets.rs with toolkit-owned SecretValue + trybuild compile-fail |
| `131172e6` | refactor(83): simplify pmcp-server-toolkit per /simplify review |
