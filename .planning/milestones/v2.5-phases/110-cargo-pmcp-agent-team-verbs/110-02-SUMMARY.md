---
phase: 110-cargo-pmcp-agent-team-verbs
plan: 02
subsystem: cargo-pmcp CLI
tags: [cli, cargo-pmcp, agent, scaffold, package, tdd, pin-tripwire]
requires:
  - pmcp-agent (0.1, Phase 108) — resolve_agent + AgentEngine + OpenAiCompatSource
  - pmcp-package (0.1, Phase 107) — AgentPackage manifest type
  - 110-01 (foundation) — agent/new.rs stub + wired deps
provides:
  - "cargo pmcp agent new <name>": scaffolds a COMPILABLE agent crate
  - templates::agent emitter (manifest-driven runner + full deps + in-scaffold pin)
  - two-level pin tripwire (internal drift-guard + emitted tests/pin.rs)
  - pub(crate) validate_crate_name (promoted for reuse, D-01a)
affects:
  - cargo-pmcp/src/commands/agent/new.rs
  - cargo-pmcp/src/commands/new.rs
  - cargo-pmcp/src/templates/mod.rs
  - cargo-pmcp/src/lib.rs
  - cargo-pmcp/Cargo.toml
  - cargo-pmcp/tests/support/scaffold_patch.rs
tech-stack:
  added:
    - semver = "1" (cargo-pmcp dep — construct AgentPackage.version)
  patterns:
    - "emitter mirrors templates/workbook_server.rs: generate() → per-file generate_* → raw fs::write"
    - "manifest BUILT from the real AgentPackage struct (not hand-written JSON) → guaranteed round-trip"
    - "manifest-driven runner: LOADS agent.package.json + resolve_agent, not a hardcoded config"
    - "internal drift-guard include_str!(pmcp-agent/Cargo.toml) + emitted in-scaffold tests/pin.rs"
    - "real-binary compile-proof integration test (cargo check + cargo test --test pin on the emitted crate)"
key-files:
  created:
    - cargo-pmcp/src/templates/agent.rs
    - cargo-pmcp/tests/scaffold_agent.rs
  modified:
    - cargo-pmcp/src/commands/agent/new.rs
    - cargo-pmcp/src/commands/new.rs
    - cargo-pmcp/src/templates/mod.rs
    - cargo-pmcp/src/lib.rs
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/tests/support/scaffold_patch.rs
decisions:
  - "manifest built from the real AgentPackage struct + serde_json::to_string_pretty (not raw JSON text) so the emitted agent.package.json cannot diverge from the schema the runner loads"
  - "runner ships a minimal NoToolsInvoker (no connectors) so the starter crate compiles + runs standalone; documented swap to ClientToolInvoker"
  - "templates_agent lib seam mirrors templates_workbook_server so the drift-guard runs under --lib (the templates tree is otherwise bin-only)"
  - "extended shared append_crates_io_patch with pmcp-agent + pmcp-package (unpublished 0.1.0) so the compile-proof test resolves them from in-repo paths"
metrics:
  duration: 38min
  completed: "2026-07-19"
  tasks: 3
  files: 8
---

# Phase 110 Plan 02: cargo-pmcp `agent new` Scaffolder Summary

Implemented `cargo pmcp agent new <name>` (CLI-01): a scaffolder that emits a
COMPILABLE agent crate — an `AgentPackage` JSON manifest, a manifest-driven
runner that LOADS that manifest and drives `AgentEngine`, a `Cargo.toml` pinning
`pmcp-agent` with the full dependency set, and an in-scaffold `tests/pin.rs`
tripwire — plus an internal drift-guard so the emitted pin cannot silently drift
from the released `pmcp-agent` version (D-05). The integration test PROVES the
emitted project compiles (`cargo check`) and its pin test passes, driving the
real built binary. Fills the plan-110-01 stub in `agent/new.rs`.

## What Was Built

**Task 1 — scaffold integration test (RED)** (`2052b7b0`)
- `tests/scaffold_agent.rs`: invokes the REAL `cargo-pmcp` binary via
  `env!("CARGO_BIN_EXE_cargo-pmcp")` in a `tempfile::tempdir()`, asserts the four
  emitted files exist, round-trips `agent.package.json` through
  `pmcp_package::AgentPackage`, then `append_crates_io_patch` + spawns a real
  `cargo check` (compile proof) and `cargo test --test pin` (tripwire proof),
  each wrapped in `ChildGuard`. A second test covers the `--force`
  destination-overwrite policy.
- Extended the shared `append_crates_io_patch` to also patch the unpublished
  `pmcp-agent` + `pmcp-package 0.1.0` crates (transitive closure for the runner).
- Failed RED (agent new was still the 110-01 `bail!` stub).

**Task 2 — templates/agent.rs emitter (GREEN unit)** (`d981e2f2`, refined `76096318`)
- `generate()` → `generate_cargo_toml` / `generate_main_rs` / `generate_manifest`
  / `generate_pin_test`, each a raw `fs::write(...).context(...)`.
- `Cargo.toml`: `pmcp-agent` (`openai-compat`), `pmcp-package "0.1"`, `tokio`
  (full), `serde_json`, `anyhow`, `async-trait`; dev-dep `toml`.
- `src/main.rs`: LOADS `agent.package.json`, `resolve_agent(&pkg,
  &EnvVarResolver::new())`, builds `OpenAiCompatSource`, drives `AgentEngine`.
- `agent.package.json`: built from the real `AgentPackage` struct then
  `serde_json::to_string_pretty` — guaranteed to round-trip.
- `tests/pin.rs`: in-scaffold tripwire asserting the `pmcp-agent` pin stays on
  the `0.1` line.
- `const PMCP_AGENT_VERSION` + `emitted_agent_version_matches_workspace_pin`
  drift-guard (`include_str!("../../../crates/pmcp-agent/Cargo.toml")`) + a
  manifest round-trip unit test + full-dep + manifest-driven wiring tests.
- Registered `templates::agent` (bin) and a `templates_agent` lib seam.

**Task 3 — wire agent new handler (GREEN e2e)** (`c22c4a2d`)
- Promoted `validate_crate_name` to `pub(crate)` (reuse, not re-implement).
- `agent/new.rs`: validates the crate name before any fs write, resolves the
  target dir (default `./<name>`), rejects a symlinked destination and refuses a
  non-empty dir unless `--force` (`ensure_destination_writable` helper keeps the
  handler under cog 25), creates `src/`, delegates to `templates::agent::generate`,
  and prints gated next-steps.
- The `scaffold_agent` integration test now passes GREEN.

## Verification

- `cargo test -p cargo-pmcp --lib templates_agent` — 5 passed (drift-guard,
  manifest round-trip, full file tree, full dep set, manifest-driven runner).
- `cargo test -p cargo-pmcp --test scaffold_agent -- --test-threads=1` — GREEN:
  real `cargo check` + `cargo test --test pin` on the emitted crate + the
  `--force` policy.
- `cargo build -p cargo-pmcp` — clean (remaining warnings are pre-existing
  pentest/deployment dead-code, out of scope).
- `cargo clippy -p cargo-pmcp --lib --bins` — no warnings from the new agent
  files (the earlier useless-`format!` on the pin template was fixed in `76096318`).
- `cargo test -p cargo-pmcp --test verb_help` — 3 passed (no regression).

## Deviations from Plan

### Auto-fixed / auto-added (Rules 2 & 3)

**1. [Rule 3 - Blocking] Extended shared `append_crates_io_patch` with pmcp-agent + pmcp-package**
- **Found during:** Task 1
- **Issue:** The compile-proof test resolves the emitted crate's deps, but
  `pmcp-agent 0.1.0` / `pmcp-package 0.1.0` are unpublished; the shared patch
  helper covered only the sql/openapi closure. A second `[patch.crates-io]` block
  is a TOML duplicate-key error, so the entries had to go in the shared helper.
- **Fix:** Added `pmcp-agent` + `pmcp-package` path overrides (harmless
  unused-patch warnings for the sql/openapi scaffolds, same as the existing
  `pmcp-openapi-server` entry).
- **Files:** `cargo-pmcp/tests/support/scaffold_patch.rs`
- **Commit:** `2052b7b0`

**2. [Rule 2 - Missing dep] Added `semver` to cargo-pmcp + `async-trait` to the emitted scaffold**
- **Found during:** Task 2
- **Issue:** `AgentPackage.version` is a `semver::Version` (cargo-pmcp needs
  semver to build the starter manifest); the emitted runner needs a `ToolInvoker`
  impl (a minimal `NoToolsInvoker`), which requires `async-trait` in the scaffold.
- **Fix:** `semver = "1"` in `cargo-pmcp/Cargo.toml`; `async-trait = "0.1"` in the
  emitted `Cargo.toml` + a documented `NoToolsInvoker` in `src/main.rs`.
- **Files:** `cargo-pmcp/Cargo.toml`, `cargo-pmcp/src/templates/agent.rs`
- **Commit:** `d981e2f2`

**3. [Rule 3 - Test seam] Added a `templates_agent` lib seam**
- **Found during:** Task 2
- **Issue:** The `templates` tree is bin-only; the plan's acceptance criterion
  runs the drift-guard via `cargo test --lib`, which only sees lib-mounted
  modules. `templates_workbook_server` already establishes the `#[path]` seam
  convention for exactly this.
- **Fix:** Mounted `#[path = "templates/agent.rs"] pub mod templates_agent;` in
  `lib.rs` (dependency-light: no `clap`/`GlobalFlags`).
- **Files:** `cargo-pmcp/src/lib.rs`
- **Commit:** `d981e2f2`

**4. [Style] Emit `tests/pin.rs` from a `&'static str` literal**
- **Found during:** Task 3 clippy verification
- **Issue:** `emitted_pin_test` used `format!` with only escaped braces (no
  interpolation) — clippy `useless_use_of_format`.
- **Fix:** Return a plain literal (byte-identical output).
- **Commit:** `76096318`

## Threat Surface

All four `mitigate` dispositions in the plan's threat model are honored:
- **T-110-02-01 (name/path tampering):** `validate_crate_name` runs before any fs
  write; a symlinked destination is rejected via `symlink_metadata`.
- **T-110-02-02 (pin drift):** `emitted_agent_version_matches_workspace_pin`
  internal drift-guard + the emitted in-scaffold `tests/pin.rs` (two levels).
- **T-110-02-03 (manifest shape drift):** the manifest is built from the real
  `AgentPackage` struct and the runner LOADS it — a round-trip unit test + the
  integration-test round-trip lock it.
- **T-110-02-04 (partial/overwrite):** a non-empty destination is refused unless
  `--force`.

No new security surface beyond the plan's threat model (no new registry
packages; `semver` was already in the lock via `pmcp-package`).

## Known Stubs

None. `agent new` is fully implemented; the emitted crate compiles and its pin
test passes (proven by the integration test). The runner's `NoToolsInvoker` is a
deliberate, documented starter default (no connectors in the starter manifest),
not an unwired stub — the manifest and runner are fully functional for the
standalone Ollama path.

## Self-Check: PASSED

- Created files exist: `cargo-pmcp/src/templates/agent.rs`,
  `cargo-pmcp/tests/scaffold_agent.rs`.
- Commits present: `2052b7b0` (RED), `d981e2f2` (emitter), `c22c4a2d` (handler),
  `76096318` (clippy fix).
