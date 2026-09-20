---
phase: 110-cargo-pmcp-agent-team-verbs
verified: 2026-07-19T00:00:00Z
status: passed
score: 4/4 must-haves verified
overrides_applied: 0
---

# Phase 110: cargo-pmcp Agent & Team Verbs Verification Report

**Phase Goal:** cargo-pmcp is the on-ramp for agents and teams, matching its server
story — `agent new`/`agent dev`, `team dev` (in-process small team from a
`TeamPackage`), and `package capture|show` (thin clients to the platform capture
API), each with version-pin tripwires. Agents deploy through the existing target
adapters (an agent-as-server is just a server binary; AgentCore is a deferred
follow-on adapter).

**Verified:** 2026-07-19
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `cargo pmcp agent new` scaffolds an agent project (AgentPackage manifest + standalone runner) with a version-pin tripwire test against `pmcp-agent` (CLI-01) | ✓ VERIFIED | Live run: `cargo pmcp agent new demo_agent_verify` emitted `Cargo.toml`, `agent.package.json` (valid `AgentPackage` JSON), `src/main.rs`, `tests/pin.rs`. `cargo test -p cargo-pmcp --test scaffold_agent -- --test-threads=1` (2 passed, 38s) proves the emitted crate `cargo check`s AND its `tests/pin.rs` passes against the real patched deps. Two-level pin tripwire confirmed: `templates/agent.rs` `const PMCP_AGENT_VERSION = "0.1.0"` + `include_str!("../../../crates/pmcp-agent/Cargo.toml")` drift-guard (matches `crates/pmcp-agent/Cargo.toml` `version = "0.1.0"`), plus the emitted `tests/pin.rs` in-scaffold tripwire. |
| 2 | `cargo pmcp agent dev` runs an agent locally against an OpenAI-compat endpoint or as a sampling-hosted server (CLI-02) | ✓ VERIFIED | Live runs: `agent dev --source fixed` → `✓ agent run (fixed) finished: Completed`; `agent dev --source openai-compat --endpoint http://localhost:19999/v1` (unreachable) → actionable `--endpoint`/`--source fixed` error; `--endpoint http://example.com/v1` (remote plain-http) → actionable `--allow-insecure-http` error at construction. `--source sampling` wired to `AgentServer::run(pmcp::StdioTransport::new())` (`commands/agent/dev.rs:251`) and covered by an automated in-process `DuplexTransport` test (`cargo test -p cargo-pmcp --test agent_dev` — 2 passed). |
| 3 | `cargo pmcp team dev` runs an in-process small team — member agents + all four reference team servers with dev backends — wired from a `TeamPackage` (CLI-03) | ✓ VERIFIED | Live run: `team dev` printed the 7-step labeled doc-review transcript (team-fs write/sync → approval-mcp ask/resolve → team-fs read → mem-mcp add → team-mcp dispatch) and `✓ doc-review flow complete — 4 hosting task(s) torn down cleanly`, fully offline. Code confirms `TeamRuntimeBuilder` (no hand-rolled composition, D-02); `--serve` uses `build_team_mcp_server` + `serve_streamable_http` (not `TeamRuntime`); `--llm` wraps `OpenAiCompatSource` in the exported `FixedSourceFactory`. `cargo test -p cargo-pmcp --test team_dev` — 3 passed (transcript + ephemeral-port `--serve` `tools/list` + `--llm` mockito). |
| 4 | `cargo pmcp package capture\|show` work as thin clients to the platform capture API with `pmcp-package = "0.1"` (caret) and a pin tripwire test against version drift (CLI-04) | ✓ VERIFIED | `cargo-pmcp/Cargo.toml` line 66: `pmcp-package = { version = "0.1", path = "../crates/pmcp-package" }`. `cargo test -p cargo-pmcp --test pmcp_package_pin` (1 passed) asserts the exact caret string. Live runs: `package show` on a non-OCI-layout path correctly rejects with an actionable error (offline, D-04); `package capture` with no configured target (isolated `HOME`) → exit 1, `Error: no platform target configured — run \`cargo pmcp configure add <name>\` first` (never a panic, D-04a). `cargo test -p cargo-pmcp --test package_show` (3 passed) proves the offline happy-path render via a real `pack_agent` fixture. `cargo test -p cargo-pmcp --test package_capture` (1) + `--lib capture_upload` (2, via `mockito`) prove the Bearer-header POST + timeout + non-2xx handling. |

**Score:** 4/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `cargo-pmcp/Cargo.toml` | pmcp-agent/pmcp-team-servers/pmcp-package deps with correct features | ✓ VERIFIED | All three present with `openai-compat`, `runtime`+`http`, and caret `"0.1"` respectively; `cargo build -p cargo-pmcp` succeeds. |
| `cargo-pmcp/src/main.rs` | `Agent`/`Team`/`Package` Commands arms + dispatch | ✓ VERIFIED | `Commands::Agent/Team/Package` variants; `execute_agent`/`execute_team`/`execute_package` dispatch via `block_on`; `Package` confirmed absent from `is_target_consuming` (grep + `main.rs:401-408`). |
| `cargo-pmcp/src/commands/agent/{mod,new,dev,run}.rs` | agent verb handlers | ✓ VERIFIED | `AgentCommand{New,Dev}`; `new.rs` validates crate name + destination policy then calls `templates::agent::generate`; `dev.rs` resolves `SourceKind` (ValueEnum) to fixed/openai-compat/sampling; `run.rs` is the lib-safe `run_fixed_source` seam (also mounted as `cargo_pmcp::agent_run`). |
| `cargo-pmcp/src/templates/agent.rs` | Agent scaffold emitter + pin drift-guard | ✓ VERIFIED | `generate()` → `generate_cargo_toml`/`generate_main_rs`/`generate_manifest`/`generate_pin_test`; `PMCP_AGENT_VERSION` drift-guard test passes. |
| `cargo-pmcp/src/commands/team/dev.rs` | team dev thin CLI over TeamRuntime | ✓ VERIFIED | Default flow uses `TeamRuntimeBuilder`; `--serve` uses `build_team_mcp_server`+`serve_streamable_http`; `--llm` uses `FixedSourceFactory(OpenAiCompatSource)`. |
| `cargo-pmcp/src/commands/package/{show,capture,kind,capture_upload}.rs` | package verbs | ✓ VERIFIED | `show.rs` dual-source (`artifactType` + media types) kind detection via pure `kind::detect_kind`; `capture.rs`/`capture_upload.rs` resolve_target + token-cache + Bearer POST + timeout + non-2xx handling. |
| `cargo-pmcp/tests/{verb_help,scaffold_agent,agent_dev,team_dev,package_show,package_capture,pmcp_package_pin}.rs` | CLI-01..04 test coverage | ✓ VERIFIED | All 7 integration-test files exist and pass (see Behavioral Spot-Checks / Probe Execution below). |
| `cargo-pmcp/examples/{agent_scaffold_and_run,team_dev_transcript}.rs` | ALWAYS EXAMPLE deliverables | ✓ VERIFIED | Both run live to completion, offline, exit 0, exercising production seams (`agent_run::run_fixed_source`, `TeamRuntimeBuilder`). |
| `cargo-pmcp/fuzz/fuzz_targets/fuzz_package_kind.rs` | ALWAYS FUZZ deliverable | ✓ VERIFIED | Builds under `+nightly cargo fuzz`; ran live for 15s / 216,358 executions with zero crashes over the untrusted manifest-parse boundary. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `main.rs` | `commands::agent::AgentCommand` | `Commands::Agent { command } => execute_agent(...)` | ✓ WIRED | Confirmed by grep + live `agent --help`/`agent new`/`agent dev` runs. |
| `commands/agent/new.rs` | `templates::agent::generate` | scaffolder delegation | ✓ WIRED | Live scaffold run + `scaffold_agent` integration test (compile + pin proof). |
| `commands/agent/dev.rs` | `commands/agent/run.rs::run_fixed_source` | fixed-source arm delegation | ✓ WIRED | `agent dev --source fixed` live run + `agent_dev` test; also reused by `examples/agent_scaffold_and_run.rs` via the `cargo_pmcp::agent_run` lib seam. |
| `commands/team/dev.rs` | `pmcp_team_servers::compose::wiring::TeamRuntimeBuilder` | default flow composition | ✓ WIRED | Live `team dev` run (7-step transcript, 4 hosting tasks torn down) + `team_dev` test. |
| `commands/team/dev.rs` | `build_team_mcp_server` + `serve_streamable_http` | `--serve` HTTP path | ✓ WIRED | `serve_exposes_team_mcp_over_http` test (ephemeral loopback port, real `tools/list`). |
| `commands/package/show.rs` | `commands::package::kind::detect_kind` → `pmcp_package::oci::unpack_*` | dual-source dispatch | ✓ WIRED | `package_show` test (3 passed) + live rejection of a non-OCI path. |
| `commands/package/capture.rs` | `configure::resolver::resolve_target` + `auth_cmd::cache` | target/token reuse (D-04a) | ✓ WIRED | Live unconfigured run (isolated HOME) → actionable `configure` error, exit 1. |

### Data-Flow Trace (Level 4)

Not applicable in the classic sense (no dashboard/UI rendering dynamic DB data), but the equivalent trust boundary — "does the CLI drive real production code, not a re-implemented/hardcoded loop" — was explicitly checked and holds:
- `agent dev --source fixed` and `examples/agent_scaffold_and_run.rs` both call the SAME `cargo_pmcp::agent_run::run_fixed_source` production seam (verified via grep + live divergent-entry-point runs producing consistent `RunOutcome::Completed`).
- `team dev` (default) and `examples/team_dev_transcript.rs` both compose via `TeamRuntimeBuilder` (no duplicated hand-rolled logic) — live output confirms real per-server tool calls (`fs__write`, `team_approval__ask_reviewer`, `mem__add`, `team_mcp__summarizer__1`), not canned strings.
- `package capture`'s token path reads a real `TokenCacheV1.entries` map (not a stub `Ok(())`), confirmed by the actionable "not authenticated"/"no platform target configured" errors on live unconfigured runs.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `agent new` scaffolds + compiles | `cargo pmcp agent new demo_agent_verify` (live) then `cargo test -p cargo-pmcp --test scaffold_agent -- --test-threads=1` | 2 passed in 38.42s (real `cargo check` + `cargo test --test pin` on emitted crate) | ✓ PASS |
| `agent dev --source fixed` | live run in scaffolded project dir | `✓ agent run (fixed) finished: Completed` | ✓ PASS |
| `agent dev --source openai-compat` unreachable endpoint | live run, `--endpoint http://localhost:19999/v1` | `Error: agent run did not complete (endpoint ... may be unreachable) — check --endpoint <url> or use --source fixed` | ✓ PASS |
| `agent dev --source openai-compat` remote plain-http | live run, `--endpoint http://example.com/v1` | `Error: remote non-HTTPS endpoint ... is blocked by default — use an https:// URL or pass --allow-insecure-http` | ✓ PASS |
| `team dev` default transcript | live run | 7-step labeled transcript + `✓ doc-review flow complete — 4 hosting task(s) torn down cleanly.` | ✓ PASS |
| `package show` on non-OCI path | live run against `agent.package.json` (not a `.pmcp` OCI layout) | `Error: ... is not an OCI image layout (.pmcp package) — missing index.json` (correct rejection, not a false positive) | ✓ PASS |
| `package capture` unconfigured | live run, isolated `HOME` | exit 1, `Error: no platform target configured — run \`cargo pmcp configure add <name>\` first` | ✓ PASS |
| `agent`/`team`/`package --help` | live run | subcommands listed correctly (`new`/`dev`; `dev`; `show`/`capture`) | ✓ PASS |
| Fuzz target links + runs | `cargo +nightly fuzz run fuzz_package_kind -- -max_total_time=15` | 216,358 executions, 0 crashes | ✓ PASS |
| Examples run | `cargo run --example agent_scaffold_and_run` / `--example team_dev_transcript` | both exit 0, offline, print production output | ✓ PASS |

### Probe Execution

No dedicated `scripts/*/tests/probe-*.sh` convention applies to this phase; the phase's own test suite (`cargo test`) serves as the probe layer and was executed directly (see below) — treated as equivalent evidence per the phase's TDD-plan structure.

| Suite | Command | Result | Status |
|-------|---------|--------|--------|
| bin unit tests | `cargo test -p cargo-pmcp --bin cargo-pmcp -- --test-threads=1` | 762 passed, 1 ignored | PASS |
| `verb_help` | `cargo test -p cargo-pmcp --test verb_help` | 3 passed | PASS |
| `scaffold_agent` | `cargo test -p cargo-pmcp --test scaffold_agent -- --test-threads=1` | 2 passed (38.42s, real compile+pin proof) | PASS |
| `agent_dev` | `cargo test -p cargo-pmcp --test agent_dev` | 2 passed | PASS |
| `team_dev` | `cargo test -p cargo-pmcp --test team_dev` | 3 passed | PASS |
| `package_show` | `cargo test -p cargo-pmcp --test package_show` | 3 passed | PASS |
| `package_capture` | `cargo test -p cargo-pmcp --test package_capture` | 1 passed | PASS |
| `pmcp_package_pin` | `cargo test -p cargo-pmcp --test pmcp_package_pin` | 1 passed | PASS |
| fuzz build+run | `cargo +nightly fuzz run fuzz_package_kind -- -max_total_time=15` | 216,358 execs, no crash | PASS |
| `cargo build -p cargo-pmcp` | build | 0 errors; only pre-existing pentest/deployment warnings (none in Phase 110 files) | PASS |
| `cargo fmt -p cargo-pmcp -- --check` | fmt | clean | PASS |
| `pmat analyze complexity --max-cognitive 25` | complexity gate | 0 violations in `commands/{agent,team,package}` or `templates/agent.rs` | PASS |
| `make lint` (CI's real gate) | root-crate clippy | clean (note: per project convention this gate lints only root `pmcp`, not `cargo-pmcp` — confirmed against project memory) | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CLI-01 | 110-01 (foundation), 110-02 (impl) | `agent new` scaffolds AgentPackage + runner + pin tripwire | ✓ SATISFIED | Live scaffold + compile-proof test + drift-guard test. |
| CLI-02 | 110-01, 110-03 | `agent dev` runs locally via openai-compat or sampling-hosted | ✓ SATISFIED | Live runs of all three `--source` values + automated sampling test. |
| CLI-03 | 110-01, 110-04 | `team dev` in-process small team from TeamPackage | ✓ SATISFIED | Live 7-step transcript run + `--serve`/`--llm` tests. |
| CLI-04 | 110-01, 110-05 | `package capture|show` thin clients + caret pin tripwire | ✓ SATISFIED | Caret-pin test + live show/capture runs + mock-endpoint capture tests. |

No orphaned requirements: REQUIREMENTS.md maps only CLI-01..04 to Phase 110, and all four are claimed (with matching evidence) across plans 110-01 through 110-05, with 110-06 additionally re-declaring all four for the ALWAYS-deliverable (example/fuzz) closure.

### Anti-Patterns Found

None blocking. Scanned all 27 files touched across the phase's commit range (`efba7631^..cb5ed4f3`, `cargo-pmcp/src`, `cargo-pmcp/tests`, `cargo-pmcp/examples`, `cargo-pmcp/fuzz`):
- No `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER` markers.
- No `unimplemented!()`/`todo!()` anywhere.
- All `panic!()` occurrences are confined to `#[test]` functions (assertion failures) or test-support helpers (`tests/support/scaffold_patch.rs`, `tests/pmcp_package_pin.rs`) — acceptable, not production stubs.
- No "not yet implemented"/"coming soon"/"not available" strings.
- The 110-01 `bail!("<verb>: implemented in plan 110-0X")` stubs are all confirmed removed (grep returns 0 matches for that pattern across the final tree).
- One informational item: `cargo clippy -p cargo-pmcp --all-targets` flags `NoopInvoker::default()` in `tests/agent_dev.rs:107` (`clippy::default_constructed_unit_struct`) — a single test-file style nit, not a production defect, and outside the project's actual CI-blocking `make lint` gate (which lints only the root `pmcp` crate, per project convention documented in the maintainer's memory notes). Not a blocker.

## Human Verification Required

None. All four Success Criteria were directly executed and observed against the real binary and the real test suite (build, tests, live CLI runs, live fuzz run, live examples) — no visual/UX/external-service judgment calls remain outstanding for this phase's scope (documentation is explicitly out of scope per 110-CONTEXT.md, deferred to Phase 111/DOCS-01..03).

## Gaps Summary

None. All four ROADMAP Success Criteria (CLI-01..04) are verified against live execution of the real `cargo-pmcp` binary plus the phase's own automated test/example/fuzz suites — not SUMMARY.md narrative. All D-01 through D-05a context decisions (nested subcommand groups, `team dev` default-offline/`--serve`/`--llm` behavior, `agent dev --source` shape and Ollama default, `package show` offline-only, `package capture` config/auth reuse, and the two-level pin tripwires) were independently confirmed in the code and via live runs. Deferred scope (AgentCore adapter, platform capture API/ECR, documentation) is correctly out of this phase per 110-CONTEXT.md and tracked under DOCS-01..03 / DEFER-01..03 in REQUIREMENTS.md, not silently dropped.

---

_Verified: 2026-07-19_
_Verifier: Claude (gsd-verifier)_
