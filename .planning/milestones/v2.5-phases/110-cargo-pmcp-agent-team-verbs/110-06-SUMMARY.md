---
phase: 110-cargo-pmcp-agent-team-verbs
plan: 06
subsystem: cargo-pmcp CLI
tags: [cli, cargo-pmcp, agent, team, package, example, fuzz, always-requirements]
requires:
  - 110-02 (templates/agent.rs — the agent new scaffold emitter)
  - 110-03 (commands/agent/run.rs — the lib-safe run_fixed_source runner seam)
  - 110-04 (commands/team/dev.rs — the offline TeamRuntime doc-review transcript)
  - 110-05 (commands/package/kind.rs — the pure manifest-parse + detect_kind leaf)
provides:
  - cargo run -p cargo-pmcp --example agent_scaffold_and_run (ALWAYS EXAMPLE for CLI-01/CLI-02)
  - cargo run -p cargo-pmcp --example team_dev_transcript (ALWAYS EXAMPLE for CLI-03)
  - cargo +nightly fuzz run fuzz_package_kind (ALWAYS FUZZ for the package parse path)
  - three #[doc(hidden)] public lib #[path] seams (templates_agent, agent_run, package_kind)
affects:
  - cargo-pmcp/src/lib.rs
  - cargo-pmcp/examples/agent_scaffold_and_run.rs
  - cargo-pmcp/examples/team_dev_transcript.rs
  - cargo-pmcp/fuzz/fuzz_targets/fuzz_package_kind.rs
  - cargo-pmcp/fuzz/Cargo.toml
tech-stack:
  added: []
  patterns:
    - example drives the PRODUCTION runner seam (run_fixed_source), never a re-implemented AgentEngine loop
    - team example drives composition entirely through TeamRuntime (D-02, no hand-rolled server spin-up)
    - libfuzzer target over the raw-bytes untrusted manifest-parse boundary (no utf8 pre-filter)
    - "#[doc(hidden)] #[path] lib seams" — internal support surface, reachable by examples/fuzz, not stable API
key-files:
  created:
    - cargo-pmcp/examples/agent_scaffold_and_run.rs
    - cargo-pmcp/examples/team_dev_transcript.rs
    - cargo-pmcp/fuzz/fuzz_targets/fuzz_package_kind.rs
  modified:
    - cargo-pmcp/src/lib.rs
    - cargo-pmcp/fuzz/Cargo.toml
decisions:
  - "The three lib seams (templates_agent, agent_run, package_kind) were already mounted by plans 110-02/03/05; this plan only marked them #[doc(hidden)] (Codex 110-06 MEDIUM) — internal support seams, not stable API"
  - "The agent example calls the PRODUCTION run_fixed_source seam (Codex 110-06 HIGH), not a re-implemented AgentEngine loop, so it exercises the real agent dev --source fixed path"
  - "The fuzz target feeds RAW bytes (no utf8 pre-filter) to artifact_type_from_manifest_json then chains detect_kind — the real package show untrusted seam, not an 8-constant string match"
metrics:
  duration: 20min
  completed: "2026-07-19"
  tasks: 3
  files: 5
requirements: [CLI-01, CLI-02, CLI-03, CLI-04]
---

# Phase 110 Plan 06: ALWAYS EXAMPLE + FUZZ deliverables Summary

Closed the CLAUDE.md "ALWAYS Requirements for New Features" gap the Phase 110 verbs
still lacked: a runnable agent scaffold+run example that drives the PRODUCTION
fixed-source runner, a runnable offline team-dev transcript example, and a libfuzzer
target over the untrusted `.pmcp` manifest-parse boundary — all reaching production
code through narrow `#[doc(hidden)]` public lib `#[path]` seams, never the bin-only
command layer.

## What Was Built

**Task 1 — three `#[doc(hidden)]` seams + the agent example** (`ed0118c8`)
- `lib.rs`: added `#[doc(hidden)]` to the three already-mounted `#[path]` seams
  (`templates_agent`, `agent_run`, `package_kind`) — Codex 110-06 MEDIUM: they are
  internal support surface reached by the example/fuzz target, not a stable API.
- `examples/agent_scaffold_and_run.rs` (new, `#[tokio::main]`): half 1 scaffolds a
  runnable agent crate via `cargo_pmcp::templates_agent::generate` into an
  auto-cleaned `tempfile::tempdir()` and prints the file tree; half 2 builds a
  `ResolvedAgentConfig::new(...)` (s50 args) and drives the PRODUCTION
  `cargo_pmcp::agent_run::run_fixed_source` runner offline (Codex 110-06 HIGH — no
  re-implemented `AgentEngine` loop), printing the terminal `RunOutcome::Completed`.

**Task 2 — `fuzz_package_kind` over the manifest-parse boundary** (`95edacdf`)
- `fuzz/fuzz_targets/fuzz_package_kind.rs` (new, `#![no_main]`): feeds RAW `&[u8]`
  (no utf8 pre-filter) to `cargo_pmcp::package_kind::artifact_type_from_manifest_json`
  and chains `detect_kind` on any extracted candidate — the real untrusted seam
  `package show` runs (bytes → artifactType → kind), never panics (threat
  T-110-05-03 / T-110-06-01).
- `fuzz/Cargo.toml`: appended the `[[bin]] fuzz_package_kind` block mirroring the
  existing entries. No new dependency — the existing `cargo-pmcp` path dep + the
  `package_kind` seam expose both fns.

**Task 3 — offline team-dev transcript example** (`169b299b`)
- `examples/team_dev_transcript.rs` (new, `#[tokio::main]`): composes the built-in
  two-member doc-review team via `TeamRuntimeBuilder` over in-memory transports with
  a `FixedSource` override (offline, deterministic), obtains the four clients, runs
  the 7-step doc-review flow printing one labeled transcript line per step, and calls
  `rt.shutdown()`. Composition is driven ENTIRELY through `TeamRuntime` (D-02 — no
  hand-rolled server spin-up), mirroring `team dev`'s default path.

## Verification

- `cargo run -p cargo-pmcp --example agent_scaffold_and_run` — exits 0; prints the
  scaffold file tree (Cargo.toml, src/main.rs, agent.package.json, tests/pin.rs) and
  the terminal `RunOutcome::Completed`. No network I/O.
- `cargo run -p cargo-pmcp --example team_dev_transcript` — exits 0; prints the
  7-step labeled transcript and "4 hosting task(s) torn down cleanly". No network I/O.
- `cargo build --manifest-path cargo-pmcp/fuzz/Cargo.toml --bin fuzz_package_kind` —
  the libfuzzer target links.
- `cargo fmt -p cargo-pmcp --check` — clean on all plan files.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The three lib seams were ALREADY mounted; only
`#[doc(hidden)]` was missing**
- **Found during:** Task 1
- **Issue:** The plan's action says to "add three `#[doc(hidden)]` `#[path]` seams",
  but plans 110-02 (`templates_agent`), 110-03 (`agent_run`), and 110-05
  (`package_kind`) had already mounted all three `#[path]` seams in `lib.rs` — they
  simply lacked the `#[doc(hidden)]` attribute the 110-06 objective calls for.
- **Fix:** Added `#[doc(hidden)]` (with a one-line rationale comment citing Codex
  110-06 MEDIUM) above each of the three existing `#[path]` seams, rather than
  re-declaring duplicate modules (which would not compile). The example/fuzz targets
  then reach the seams exactly as the plan intended.
- **Files modified:** `cargo-pmcp/src/lib.rs`
- **Commit:** `ed0118c8`

## Threat Surface

Per the plan's threat model, the one `mitigate` disposition is honored:
- **T-110-06-01 (DoS — adversarial manifest bytes):** `fuzz_package_kind` drives the
  pure `artifact_type_from_manifest_json` + `detect_kind` path over raw arbitrary
  bytes, proving it never panics/hangs — covering `package show`'s untrusted-parse
  boundary (the real seam, Codex 110-06 MEDIUM).
- **T-110-06-02 (example filesystem writes, accept):** both examples write only into
  `tempfile::tempdir()` dirs that auto-clean; no user path input, no network.
- **T-110-06-SC (dependency additions, accept):** no new registry packages — the
  seams re-mount in-repo leaves and the fuzz crate reuses its existing `cargo-pmcp`
  path dep.

No new security surface beyond the plan's threat model.

## Known Stubs

None. All three ALWAYS deliverables are runnable and exercise production code paths.

## Self-Check: PASSED

- Created files exist: `examples/agent_scaffold_and_run.rs`,
  `examples/team_dev_transcript.rs`, `fuzz/fuzz_targets/fuzz_package_kind.rs` — all
  FOUND on disk.
- Task commits present in git history: `ed0118c8`, `95edacdf`, `169b299b` — all FOUND.
