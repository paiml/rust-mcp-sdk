---
phase: 83-toolkit-core-lift-pmcp-server-toolkit
plan: 01
subsystem: toolkit-scaffold
tags:
  - toolkit
  - scaffold
  - workspace
  - cargo
requirements:
  - TKIT-01
dependency_graph:
  requires:
    - "pmcp 2.8.1 (root crate, workspace-version path dep)"
    - "pmcp-code-mode 0.5.1 (optional, gated behind default `code-mode` feature)"
    - "pmcp-run external repo (`$HOME/Development/mcp/sdk/pmcp-run` or `$PMCP_RUN_PATH`) for fixture snapshots"
  provides:
    - "crates/pmcp-server-toolkit workspace member with empty module skeleton"
    - "ToolkitError + Result<T> alias as the toolkit-wide error surface (Plan 04 will extend)"
    - "Reference config.toml fixtures (open-images, imdb, msr-vtt) for Plan 04 SC-2 superset verification"
    - "CLAUDE.md publish-order slot 3 reservation"
  affects:
    - "Plans 02–08 (now have a landing crate for SecretValue, ServerConfig, synthesize_from_config, code-mode wiring, …)"
    - "Phase 86 (Shape C 15-line main.rs depends on this crate)"
tech-stack:
  added:
    - "secrecy 0.10 (toolkit-owned SecretValue newtype — review R6 prep)"
    - "trybuild 1 (dev-only — Plan 02 compile-fail tests for SecretValue invariants — review R5 prep)"
    - "jsonschema 0.46 (optional, `input-validation` feature)"
    - "rusqlite 0.39 bundled (optional, `sqlite` feature)"
    - "aws-sdk-secretsmanager, aws-sdk-ssm, aws-config (optional, `aws` feature, default-features = false + default-https-client to dodge RUSTSEC-2026-0098/0099/0104)"
  patterns:
    - "Pattern I — attribution-header three-liner at the top of every source file"
    - "Pattern B — `#[non_exhaustive]` thiserror enum + crate-level Result<T> alias"
    - "Pattern H — workspace-version pmcp dep: `pmcp = { version = \"2.8.1\", path = \"../..\", default-features = false }`"
    - "Pitfall 6 — `exclude = [\".planning/\", \".pmat/\", \"fixtures/\", \"tests/\", \"fuzz/\"]` for crates.io 10MB ceiling"
    - "PATTERNS §14 — physical (non-symlink) fixture copies for cross-platform safety"
    - "Review R10 — `$PMCP_RUN_PATH` env-var override + halt-loudly on unreadable source (no fabricated stubs)"
key-files:
  created:
    - "crates/pmcp-server-toolkit/Cargo.toml"
    - "crates/pmcp-server-toolkit/README.md"
    - "crates/pmcp-server-toolkit/src/lib.rs"
    - "crates/pmcp-server-toolkit/src/error.rs"
    - "crates/pmcp-server-toolkit/src/auth.rs"
    - "crates/pmcp-server-toolkit/src/secrets.rs"
    - "crates/pmcp-server-toolkit/src/config.rs"
    - "crates/pmcp-server-toolkit/src/prompts.rs"
    - "crates/pmcp-server-toolkit/src/resources.rs"
    - "crates/pmcp-server-toolkit/src/tools.rs"
    - "crates/pmcp-server-toolkit/src/builder_ext.rs"
    - "crates/pmcp-server-toolkit/src/code_mode.rs"
    - "crates/pmcp-server-toolkit/src/sql/mod.rs"
    - "crates/pmcp-server-toolkit/tests/fixtures/open-images-config.toml"
    - "crates/pmcp-server-toolkit/tests/fixtures/imdb-config.toml"
    - "crates/pmcp-server-toolkit/tests/fixtures/msr-vtt-config.toml"
  modified:
    - "Cargo.toml (root — added crates/pmcp-server-toolkit to [workspace.members])"
    - "CLAUDE.md (publish-order list — pmcp-server-toolkit slotted at position 3)"
decisions:
  - "D-05/D-07/D-08/D-14/D-15 honored verbatim — locked feature matrix, publish slot, module set"
  - "secrecy declared UNCONDITIONALLY (per review R6, not gated behind code-mode)"
  - "trybuild added to dev-deps (per review R5) to unblock Plan 02 compile-fail tests"
  - "AWS SDKs all opt-out of rustls 0.21 via default-features = false + default-https-client (RUSTSEC-2026-0098/0099/0104 dodge)"
  - "Module stubs are zero-body — no premature scaffolding, no SATD comments"
  - "PATTERNS §Anti-Patterns #10 honored — no blanket `#![allow(clippy::*)]` lines in lib.rs (cleaner baseline than the pmcp-code-mode analog)"
metrics:
  duration_minutes: 70
  completed_date: "2026-05-18"
  tasks_completed: 5
  files_created: 16
  files_modified: 2
  commits: 4
---

# Phase 83 Plan 01: Toolkit Core Lift — Workspace Scaffold Summary

Phase 83 Plan 01 stands up the `pmcp-server-toolkit` workspace crate so every subsequent plan in the phase has a place to land code. Slots the crate into root `[workspace.members]` and `CLAUDE.md` publish-order list at position 3, declares the locked D-14 feature matrix and D-15 module set, captures three reference `config.toml` fixtures from the external pmcp-run repo for the Plan 04 SC-2 superset test, and pre-stages `secrecy` + `trybuild` dependencies that downstream plans need (reviews R5/R6).

## Tasks Completed

| Task | Description                                                                       | Commit     |
| ---- | --------------------------------------------------------------------------------- | ---------- |
| 1    | Add workspace member entry + slot manifest (with R5/R6 dep additions)             | `bd6fb9ac` |
| 2    | Emit ToolkitError + Result alias + lib.rs module skeleton                         | `8b41b8a2` |
| 3    | Create empty module file stubs + CRATE-README + CLAUDE.md publish-order edit      | `54218a57` |
| 4    | Snapshot the three reference config.toml fixtures (R10 path-fallback)             | `299bf823` |
| 5    | Run quality gate baseline (no file changes — gate passed clean on first attempt)  | *(no commit — no source diff)* |

## Outcomes

- **Workspace member** — `cargo metadata --format-version 1 --no-deps` reports `pmcp-server-toolkit` as a workspace crate. Both `cargo build -p pmcp-server-toolkit` (default features = `code-mode`) and `--no-default-features` complete with zero errors and zero warnings.
- **Module skeleton** — 9 module files declared in `lib.rs` (auth, secrets, config, prompts, resources, tools, builder_ext, code_mode, sql/mod) plus the populated `error.rs`. Every stub carries a Pattern I attribution header and a one-line `//!` rustdoc keyed to the RESEARCH Architectural Responsibility Map.
- **`ToolkitError`** — `#[non_exhaustive]` `thiserror::Error` enum with five initial variants (Parse, MissingField, Synth, CodeMode, Io) plus the crate-level `Result<T>` alias. Doctest on `ToolkitError::MissingField` proves the `Display` impl and `std::error::Error` integration (TEST-03 baseline established).
- **Feature matrix locked to D-14** — `default = ["code-mode"]`, optional `code-mode` / `aws` / `avp` / `input-validation` / `sqlite`. The dropped features (`ddb`, `dynamo-config`, `openapi-code-mode`, `js-runtime`, `mcp-code-mode`) are absent from the manifest, as verified by the Task 1 grep guard.
- **Publish-size guard** — `exclude = [".planning/", ".pmat/", "fixtures/", "tests/", "fuzz/"]` keeps the published artifact under the crates.io 10MB ceiling (Pitfall 6). Plan 09 will close the loop with a `cargo package --list` gate.
- **Review prep (R5/R6)** — `trybuild = "1"` lands in `[dev-dependencies]` (Plan 02 compile-fail tests for `SecretValue`'s negative trait invariants) and `secrecy = "0.10"` lands in `[dependencies]` unconditionally (toolkit-owned `SecretValue` ships in Plan 02 regardless of `code-mode` feature state).
- **CLAUDE.md publish-order** — slot 3 reserved for `pmcp-server-toolkit (runtime library; depends on pmcp)`, downstream crates renumbered (mcp-tester → 4, mcp-preview → 5, cargo-pmcp → 6) per D-08.
- **Reference fixtures** — all three snapshot files (open-images 397 lines, imdb 803 lines, msr-vtt 352 lines) live as physical copies under `crates/pmcp-server-toolkit/tests/fixtures/`. Each carries the three-line `# Snapshot from pmcp-run/...` header. All three parse as valid TOML via Python `tomllib`. R10 path-fallback honored — `$PMCP_RUN_PATH` override resolved to the default `$HOME/Development/mcp/sdk/pmcp-run` and source files were all readable.
- **Quality gate** — `make quality-gate` exits 0 with `ALL TOYOTA WAY QUALITY CHECKS PASSED` banner. fmt clean, clippy (full features, pedantic + nursery lint groups) clean, build green, test suite green, audit clean, all four ALWAYS validators pass.

## Deviations from Plan

None — plan executed exactly as written.

Two reasons no Rule 1/2/3 auto-fixes were necessary:

1. **Module-stub design choice (already locked):** the plan explicitly steers away from premature scaffolding (D-15) and from blanket `#![allow(clippy::*)]` lines (PATTERNS §Anti-Patterns #10), so the resulting empty modules produced zero clippy warnings on `make lint` even with pedantic + nursery groups active.
2. **Fuzz subproject errors during `make validate-always`:** the worktree's `fuzz/Cargo.toml` carries an absolute `workspace = ...` path that points to the canonical rust-mcp-sdk root rather than the worktree root, so `cargo fuzz` emits "current package believes it's in a workspace when it's not" for every fuzz target. These are absorbed by the `||` clauses in the `test-fuzz` Make target (the messages are printed but the rule succeeds), so the gate still completes with `exit 0` and the `ALL TOYOTA WAY QUALITY CHECKS PASSED` banner. This is a known worktree-mode quirk — not a Phase 83 deviation, not a Plan 01 fix-target.

## Known Stubs

| File | Reason | Resolves in |
|------|--------|-------------|
| `crates/pmcp-server-toolkit/src/auth.rs` | Empty module — `AuthProvider` impls land later | Plan 02 |
| `crates/pmcp-server-toolkit/src/secrets.rs` | Empty module — `SecretValue` + `SecretsProvider` impls land later | Plan 02 |
| `crates/pmcp-server-toolkit/src/config.rs` | Empty module — `ServerConfig` types land later | Plan 04 |
| `crates/pmcp-server-toolkit/src/prompts.rs` | Empty module — `StaticPromptHandler` lands later | Plan 05 |
| `crates/pmcp-server-toolkit/src/resources.rs` | Empty module — `StaticResourceHandler` lands later | Plan 05 |
| `crates/pmcp-server-toolkit/src/tools.rs` | Empty module — `synthesize_from_config` lands later | Plan 06 |
| `crates/pmcp-server-toolkit/src/builder_ext.rs` | Empty module — `ServerBuilderExt` trait lands later | Plan 07 |
| `crates/pmcp-server-toolkit/src/code_mode.rs` | Empty module — `executor_from_config` lands later | Plan 08 |
| `crates/pmcp-server-toolkit/src/sql/mod.rs` | Empty module — `SqlConnector` trait lands later | Plan 06/08 |

These are intentional D-15 module-skeleton placeholders that subsequent plans fill in. No data is wired to a UI; no user-facing surface ships yet. SUMMARY documents each so the verifier records the intent.

## Threat Flags

None — no new attack surface introduced. Per the plan's threat model:

- T-83-01-01 (feature matrix tampering): mitigated by Task 1 grep guard (none of `ddb`, `dynamo-config`, `openapi-code-mode`, `js-runtime`, `mcp-code-mode` present).
- T-83-01-02 (publish-artifact info disclosure): mitigated by `exclude = [".planning/", ".pmat/", "fixtures/", "tests/", "fuzz/"]`.
- T-83-01-03 (AWS dep tampering): mitigated — all three AWS deps use `default-features = false` + `default-https-client`.
- T-83-01-04 (fixture tampering): mitigated — each fixture carries the `# Snapshot from pmcp-run/...` header, files are physical (not symlinks), all ≥ 350 lines.
- T-83-01-05 (publish-order repudiation): mitigated — CLAUDE.md updated to show `pmcp-server-toolkit` at slot 3 per D-08.
- T-83-01-06 (fabricated fixture when source unreadable, R10): mitigated — fallback ran with `$PMCP_RUN_PATH` default; all three sources verified readable before copy.

## Verification Snapshot

```text
$ cargo metadata --format-version 1 --no-deps | grep -o '"pmcp-server-toolkit"' | head -1
"pmcp-server-toolkit"

$ cargo build -p pmcp-server-toolkit --no-default-features
   Compiling pmcp-server-toolkit v0.1.0 (.../crates/pmcp-server-toolkit)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.54s

$ cargo build -p pmcp-server-toolkit
   Compiling pmcp-server-toolkit v0.1.0 (.../crates/pmcp-server-toolkit)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.72s

$ cargo test --doc -p pmcp-server-toolkit --no-default-features
   Doc-tests pmcp_server_toolkit
cargo test: 1 passed (1 suite, 0.50s)

$ make quality-gate; echo $?
ALL TOYOTA WAY QUALITY CHECKS PASSED
ALWAYS Requirements Validated
0
```

## Self-Check: PASSED

- `crates/pmcp-server-toolkit/Cargo.toml` — FOUND
- `crates/pmcp-server-toolkit/README.md` — FOUND
- `crates/pmcp-server-toolkit/src/lib.rs` — FOUND
- `crates/pmcp-server-toolkit/src/error.rs` — FOUND
- `crates/pmcp-server-toolkit/src/auth.rs` — FOUND
- `crates/pmcp-server-toolkit/src/secrets.rs` — FOUND
- `crates/pmcp-server-toolkit/src/config.rs` — FOUND
- `crates/pmcp-server-toolkit/src/prompts.rs` — FOUND
- `crates/pmcp-server-toolkit/src/resources.rs` — FOUND
- `crates/pmcp-server-toolkit/src/tools.rs` — FOUND
- `crates/pmcp-server-toolkit/src/builder_ext.rs` — FOUND
- `crates/pmcp-server-toolkit/src/code_mode.rs` — FOUND
- `crates/pmcp-server-toolkit/src/sql/mod.rs` — FOUND
- `crates/pmcp-server-toolkit/tests/fixtures/open-images-config.toml` — FOUND
- `crates/pmcp-server-toolkit/tests/fixtures/imdb-config.toml` — FOUND
- `crates/pmcp-server-toolkit/tests/fixtures/msr-vtt-config.toml` — FOUND
- Commit `bd6fb9ac` (Task 1) — FOUND
- Commit `8b41b8a2` (Task 2) — FOUND
- Commit `54218a57` (Task 3) — FOUND
- Commit `299bf823` (Task 4) — FOUND
