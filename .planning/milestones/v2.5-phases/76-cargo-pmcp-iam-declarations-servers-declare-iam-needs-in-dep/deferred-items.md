# Phase 76 — Deferred items

Out-of-scope findings discovered during plan execution that the scope-boundary
rule says NOT to auto-fix. Tracked here for the phase planner to triage.

## Pre-existing clippy errors (discovered 2026-04-22, Wave 1 / 76-01)

`cargo clippy -p cargo-pmcp --all-targets -- -D warnings` fails with 20 errors
on `c4b9fbe0f1fc8f7f9421c6df8984a9d9ac2b4399` (the Wave-1 base commit). All
errors are in files NOT modified by 76-01:

- `cargo-pmcp/src/pentest/mod.rs:20-23` — 4 unused `pub use` re-exports
- `cargo-pmcp/src/pentest/attacks/data_exfiltration.rs:615,627,631` — 3×
  `clippy::manual_contains` violations (use `.contains()` not `iter().any()`)
- `cargo-pmcp/src/pentest/payloads/mod.rs:12,28,42` — dead code
- `cargo-pmcp/src/pentest/payloads/injection.rs:21` — dead `curated_injection_payloads`
- `cargo-pmcp/src/pentest/schema_utils.rs:47` — dead `truncate_for_evidence`
- `cargo-pmcp/src/pentest/config.rs:94` — unread `quiet` field
- `cargo-pmcp/src/commands/pentest.rs:201` — wildcard-pattern-covers-any-other
- `cargo-pmcp/src/secrets/mod.rs:62` — unused `SecretResolution` re-export
- `cargo-pmcp/src/deployment/metadata.rs:941` — unused `std::io::Write`
- `cargo-pmcp/src/deployment/config.rs:494` — collapsible-if (pre-76-01 code,
  introduced 2025-12-15 commit 87575dc0e)

**Why deferred:** per execution scope-boundary rule, Wave 1 only auto-fixes
issues its own changes caused. None of the above are in files 76-01 modified
(`deployment/config.rs` line 494 is pre-existing; my new additions are at
lines 33 and 744+). The broader CLAUDE.md `make quality-gate` requirement
implies these need to be fixed at the phase boundary. Flag for:

- (a) a pre-phase cleanup task, or
- (b) scoping into Wave 5 (which already owns the phase-boundary quality gate).

**Reproduction:**
```bash
cargo clippy -p cargo-pmcp --all-targets -- -D warnings
```
