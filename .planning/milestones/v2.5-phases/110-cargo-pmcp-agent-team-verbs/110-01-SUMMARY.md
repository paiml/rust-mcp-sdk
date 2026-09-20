---
phase: 110-cargo-pmcp-agent-team-verbs
plan: 01
subsystem: cargo-pmcp CLI
tags: [cli, cargo-pmcp, agent, team, package, scaffolding]
requires:
  - pmcp-agent (0.1, Phase 108)
  - pmcp-team-servers (0.1, Phase 109)
  - pmcp-package (0.1, Phase 107)
provides:
  - cargo pmcp agent/team/package command groups (stubbed handlers)
  - three workspace-crate dependencies wired into cargo-pmcp
  - disjoint stub files for the four Wave-2 verb plans (110-02..110-05)
affects:
  - cargo-pmcp/Cargo.toml
  - cargo-pmcp/src/commands/mod.rs
  - cargo-pmcp/src/main.rs
tech-stack:
  added:
    - pmcp-agent = { version = "0.1", features = ["openai-compat"] }
    - pmcp-team-servers = { version = "0.1", features = ["runtime", "http"] }
    - pmcp-package = { version = "0.1" }
  patterns:
    - clap Subcommand command-group enum with async execute (mirrors workbook, D-01)
    - sync main.rs dispatch wrapper owning a tokio runtime + block_on (mirrors execute_landing)
    - actionable anyhow::bail! stubs (never panic/unimplemented/todo)
key-files:
  created:
    - cargo-pmcp/src/commands/agent/mod.rs
    - cargo-pmcp/src/commands/agent/new.rs
    - cargo-pmcp/src/commands/agent/dev.rs
    - cargo-pmcp/src/commands/team/mod.rs
    - cargo-pmcp/src/commands/team/dev.rs
    - cargo-pmcp/src/commands/package/mod.rs
    - cargo-pmcp/src/commands/package/show.rs
    - cargo-pmcp/src/commands/package/capture.rs
    - cargo-pmcp/tests/verb_help.rs
  modified:
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/src/commands/mod.rs
    - cargo-pmcp/src/main.rs
decisions:
  - "http feature transitively enables member-llm; do NOT list member-llm explicitly (minimal feature graph)"
  - "pmcp-package pinned caret \"0.1\" exactly (CLI-04 / D-04b tripwire), not =0.1.0"
  - "package capture uses a capture-local --target, NOT a top-level GlobalFlags flag; Package stays OUT of is_target_consuming (Codex MEDIUM)"
  - "stub params underscore-prefixed so the zero-warning gate passes; Wave-2 plans rename to args/global_flags (Codex HIGH)"
metrics:
  duration: 44min
  completed: "2026-07-19"
  tasks: 3
  files: 12
---

# Phase 110 Plan 01: cargo-pmcp Agent & Team Verbs — Shared Foundation Summary

Wired the shared CLI foundation for Phase 110: added the three already-shipped
workspace crates (`pmcp-agent`, `pmcp-team-servers`, `pmcp-package`) as
cargo-pmcp dependencies with their load-bearing feature flags, created the
`agent`/`team`/`package` nested subcommand groups (mirroring the existing
`workbook` group with async `execute`), and registered three `enum Commands`
arms with async dispatch wrappers in `main.rs`. Every verb handler is an
actionable `anyhow::bail!` stub, so the whole binary compiles green and each
verb is reachable — the four Wave-2 verb plans (110-02..110-05) fill the stubs
in parallel over disjoint files.

## What Was Built

**Task 1 — dependencies + version bump** (`b2dff8de`)
- `pmcp-agent` with `openai-compat` (the default `agent dev` completion source)
- `pmcp-team-servers` with `runtime` + `http` (`http` transitively enables
  `member-llm`, verified in the feature tree)
- `pmcp-package` pinned caret `"0.1"` (CLI-04 tripwire)
- `[package] version` bumped `0.17.4` → `0.18.0`

**Task 2 — command groups + stubbed handlers** (`7e732c8c`)
- `agent` group: `AgentCommand { New, Dev }`, async `execute`
- `team` group: `TeamCommand { Dev }`, async `execute`
- `package` group: `PackageCommand { Show, Capture }`, async `execute`
- Five stub handler files, each defining the pivotal flags for downstream plans;
  bodies are single `anyhow::bail!` calls with underscore-prefixed unused params
- `capture.rs` carries the capture-local `#[arg(long)] target: Option<String>`

**Task 3 — main.rs wiring + --help test** (`a2381423`)
- Three `enum Commands` variants mirroring the `Workbook` arm shape
- Three dispatch arms via `execute_agent`/`execute_team`/`execute_package`, each
  a sync fn owning a `tokio::runtime::Runtime` + `block_on` (copies
  `execute_landing`'s shape)
- `Package` deliberately kept out of `is_target_consuming` (capture resolves its
  own target explicitly)
- `tests/verb_help.rs`: asserts `agent`/`team`/`package --help` exit 0 and list
  their subcommands

## Verification

- `cargo build -p cargo-pmcp` — compiles with the three new deps + command groups
- `cargo tree` — all three crates resolve; `member-llm` present transitively via `http`
- `cargo clippy -p cargo-pmcp --all-targets` — no unused-argument warnings from stubs
- `cargo test -p cargo-pmcp --test verb_help` — 3 passed (the three --help surfaces)
- `cargo test -p cargo-pmcp --lib` — 453 passed, 1 ignored (no regression)
- Pre-commit quality gate passed on all three commits

## Deviations from Plan

None — plan executed exactly as written.

## Threat Surface

Per the plan's threat model, both `mitigate` dispositions are honored:
- **T-110-01 (DoS):** every stub is a single `anyhow::bail!` — grep confirms no
  `panic!`/`unimplemented!`/`todo!` in any of the eight new module files.
- **T-110-02 (EoP):** `Package` is NOT target-consuming, so it cannot clobber
  `PMCP_TARGET`/`AWS_*` env for unrelated commands.

No new security surface beyond the plan's threat model (all three added deps are
first-party in-repo workspace crates; zero new registry packages).

## Known Stubs

All five verb handlers are intentional stubs that `bail!` with an actionable
"implemented in plan 110-0X" message. This is by design — this plan establishes
the CLI surface + dependency wiring once so the Wave-2 verb plans own disjoint
files:

| Handler | File | Resolved by |
|---------|------|-------------|
| `agent new` | `commands/agent/new.rs` | Plan 110-02 |
| `agent dev` | `commands/agent/dev.rs` | Plan 110-03 |
| `team dev` | `commands/team/dev.rs` | Plan 110-04 |
| `package show` | `commands/package/show.rs` | Plan 110-05 |
| `package capture` | `commands/package/capture.rs` | Plan 110-05 |

## Follow-ups / Release notes

- Release-checkpoint (not a Phase-110 task): at release time run
  `cargo publish --dry-run -p cargo-pmcp` after the three new deps are on
  crates.io; cargo-pmcp now publishes AFTER all three (design §5). The release
  workflow skips already-published crates gracefully.

## Self-Check: PASSED

All created files exist on disk; all three task commits (b2dff8de, 7e732c8c, a2381423) are present in git history.
