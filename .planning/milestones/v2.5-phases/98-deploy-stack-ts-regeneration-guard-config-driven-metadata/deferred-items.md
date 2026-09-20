# Phase 98 — Deferred Items

Out-of-scope discoveries logged during execution. Do NOT fix as part of this phase.

## Pre-existing flaky/failing test (NOT caused by Plan 98-02)

- **Test:** `cargo_pmcp` lib test `test_support_cache::proptests::normalize_round_trip_idempotent`
- **Location:** `cargo-pmcp/src/commands/auth_cmd/cache.rs:419`
- **Symptom:** proptest panics on `--lib` runs.
- **Evidence it is pre-existing:** reproduced on clean HEAD with Plan 98-02 changes set aside; failure is in an auth-cache normalization proptest with no relationship to the deploy/stack.ts guard work.
- **Scope:** SCOPE BOUNDARY — pre-existing failure in an unrelated file. Left untouched per executor scope rules. Flag for a future auth-cmd fix.
