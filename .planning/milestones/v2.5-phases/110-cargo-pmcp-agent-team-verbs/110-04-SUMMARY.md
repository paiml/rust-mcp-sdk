---
phase: 110-cargo-pmcp-agent-team-verbs
plan: 04
subsystem: cargo-pmcp CLI
tags: [cli, cargo-pmcp, team, teamruntime, http, llm, doc-review]
requires:
  - pmcp-team-servers (0.1, Phase 109) — TeamRuntimeBuilder + team-mcp binary recipe
  - pmcp-agent (0.1, Phase 108) — OpenAiCompatSource + FixedSourceFactory
  - pmcp-package (0.1, Phase 107) — TeamPackage / AgentPackage fixtures
  - 110-01 (foundation stub: DevArgs + bail! handler)
provides:
  - "cargo pmcp team dev (CLI-03): offline 7-step doc-review transcript"
  - "team dev --serve: team-mcp over HTTP via the shipped binary's public recipe"
  - "team dev --llm <endpoint>: real OpenAiCompatSource via the exported FixedSourceFactory"
affects:
  - cargo-pmcp/src/commands/team/dev.rs
  - cargo-pmcp/tests/team_dev.rs
tech-stack:
  added: []
  patterns:
    - default flow delegates composition entirely to TeamRuntime (D-02) — no hand-rolled server spin-up
    - --serve reuses the team-mcp binary's member-wiring loop + build_team_mcp_server + serve_streamable_http (no upstream change, no TeamRuntime)
    - --llm constructs + validates the source ONCE, then wraps it in the exported FixedSourceFactory (the CompletionSourceFactory trait is sync + infallible — a custom fallible factory is the wrong shape)
    - built-in doc-review fixture synthesized into tempdirs (D-02 locked default), guards held for the run's lifetime
    - each branch is a small helper (cognitive complexity <=25)
key-files:
  created:
    - cargo-pmcp/tests/team_dev.rs
  modified:
    - cargo-pmcp/src/commands/team/dev.rs
decisions:
  - "default transcript composition is 100% TeamRuntime (D-02): TeamRuntimeBuilder + the four *_client() accessors + rt.shutdown()"
  - "--serve uses the PUBLIC team-mcp binary recipe on 127.0.0.1:<--port>, NOT TeamRuntime (which exposes only in-memory clients) and with NO upstream API change (Codex 110-04 HIGH)"
  - "--llm wraps a validated OpenAiCompatSource in the ALREADY-EXPORTED FixedSourceFactory — sync/infallible factory shape, not a custom fallible factory (Codex 110-04 HIGH)"
  - "behavioral tests characterize the composable primitives directly because commands::* is bin-only (110-03 note) — an integration test cannot reach the bail! stub"
metrics:
  duration: 40min
  completed: "2026-07-19"
  tasks: 2
  files: 2
---

# Phase 110 Plan 04: `cargo pmcp team dev` (CLI-03) Summary

Implemented `cargo pmcp team dev` (CLI-03) as a thin CLI over the Phase 108/109
primitives. The default flow composes the two-member doc-review team in ONE
process via `TeamRuntimeBuilder` over in-memory transports on an offline
`FixedSource`, then walks the 7-step doc-review flow (fs write/publish → ask
reviewer → resolve verdict → read → remember → member dispatch) printing a
labeled transcript, and shuts down cleanly — fully offline and deterministic.
`--serve` exposes team-mcp over HTTP by reusing the shipped `team-mcp` binary's
PUBLIC serve recipe (`build_team_mcp_server` over the member-wiring loop, then
`serve_streamable_http`) on `127.0.0.1:<--port>`, never through `TeamRuntime` and
with no upstream API change. `--llm <endpoint>` swaps the offline source for a
validated `OpenAiCompatSource` wrapped in the already-exported
`FixedSourceFactory` (the correct sync/infallible factory shape). Fills the
110-01 stub in `team/dev.rs`.

## What Was Built

**Task 1 — behavioral tests (RED/spec, `bf50b8f3`)**
- `cargo-pmcp/tests/team_dev.rs` with three `#[tokio::test]`s, all offline/loopback:
  - `transcript_drives_seven_step_doc_review_offline`: `TeamRuntimeBuilder` +
    `with_completion_override(fixed_override())` drives the 7 steps in order and
    asserts `rt.shutdown()` joins every hosted task (`joined == hosted`).
  - `serve_exposes_team_mcp_over_http`: the public team-mcp binary recipe
    (`build_team_mcp_server` + `StreamableHttpServer` on `127.0.0.1:0`); an MCP
    `Client` over `StreamableHttpTransport` completes `tools/list` and sees the
    two `team_mcp__<member>` tools, then the serve task is aborted.
  - `llm_drives_against_mock_endpoint`: an `OpenAiCompatSource` pointed at a
    `mockito` endpoint (canned end-turn `chat/completions`) wrapped in
    `FixedSourceFactory`; a member dispatch runs the mock-backed loop, and the
    test asserts `mock.assert_async()` + a terminal related-task pointer.

**Task 2 — implementation (GREEN, `05b26983`)**
- `cargo-pmcp/src/commands/team/dev.rs`: replaced the `bail!` stub.
  - `LoadedTeam::load`: an explicit `--package` (members resolved from
    `--data-dir`, default `./team-mcp-data`) or the built-in doc-review fixture
    synthesized into tempdirs (guards held for the run).
  - `run_transcript`: `TeamRuntimeBuilder` + the four `*_client()` accessors +
    the 7-step flow + `rt.shutdown()`; small per-step helpers, output gated
    behind `global_flags.should_output()`.
  - `serve_team_mcp`: the member-wiring loop (`resolve_agent` /
    `resolve_member_factory(None)` / `MemberHandle::spawn_from_package`) →
    `build_team_mcp_server` → `serve_streamable_http`, running under a
    `tokio::select!` with `ctrl_c()` for graceful stop.
  - `completion_factory` / `build_llm_source`: parse `--llm` with `url::Url`,
    build `OpenAiCompatSource::with_options` once (Decode → actionable
    `--allow-insecure-http` bail), key from `--llm-api-key-env` as a
    `SecretString`, wrapped in `FixedSourceFactory`.
  - Added `--llm-api-key-env` and `--model` to `DevArgs` for the `--llm` branch.

## Verification

- `cargo test -p cargo-pmcp --test team_dev` — 3 passed (transcript + `--serve`
  ephemeral-port tools/list + `--llm` mockito smoke).
- `cargo build -p cargo-pmcp` — the `--serve` (http) and `--llm` (member-llm via
  http) branches compile.
- `cargo run --bin cargo-pmcp -- team dev` — prints the labeled 7-step transcript
  and `✓ doc-review flow complete — 4 hosting task(s) torn down cleanly.` offline.
- `cargo clippy -p cargo-pmcp --all-targets` — no new warnings from `team/dev.rs`
  or `team_dev.rs` (residual warnings are pre-existing in unrelated pentest/gcp
  modules — out of scope).
- `pmat analyze complexity --max-cognitive 25` — no violations in `team/dev.rs`.

## Deviations from Plan

### TDD gate nuance (documented, not a code change)

The plan's Task 1 acceptance criterion "Before Task 2, `cargo test ... team_dev`
fails (RED)" could not be met literally: `commands::*` is bin-only (established in
110-01/110-03), so an integration test cannot import or call the `team dev`
handler and therefore cannot RED-fail on its `bail!` stub. The three tests instead
characterize the composable primitives the handler assembles
(`TeamRuntimeBuilder`, `build_team_mcp_server` + `serve_streamable_http`,
`OpenAiCompatSource` + `FixedSourceFactory`) — these already exist from Phases
108/109, so they pass immediately. The *feature* (the CLI wiring) did not exist
until Task 2. This is an executable-specification suite, not a classic stub-RED;
the GREEN implementation composes exactly the primitives the tests pin. No lib
seam (à la 110-03's `cargo_pmcp::agent_run`) was added because the plan did not
call for one and the tests fully cover the three mechanisms.

### Rule 2 — actionable errors on a mismatched `--package`

The transcript flow requires all four runtime clients (team-fs, approval-mcp,
mem-mcp, team-mcp). A user-supplied `--package` that does not attach one of them
would otherwise panic on `.expect(...)`. Added `require_client`, which bails with
an actionable message ("the team package does not attach {name}; the built-in
doc-review flow needs ≥2 members, a human role, and team-fs + mem-mcp opt-ins")
instead of panicking.

## Threat Surface

All three `mitigate` dispositions in the plan's threat register are honored:
- **T-110-04-01 (info disclosure, `--serve`):** reuses the SDK-owned
  `serve_streamable_http` recipe (DNS-rebinding/CORS/headers) and binds loopback
  `127.0.0.1:<port>`; the default (no `--serve`) stays fully in-process.
- **T-110-04-02 (tampering/EoP, `--llm` URL):** `--llm` is parsed with `url::Url`
  and constructed via `OpenAiCompatSource::with_options`, whose `validate_endpoint`
  rejects remote plain-http unless `--allow-insecure-http` (actionable bail).
- **T-110-04-03 (info disclosure, `--llm` key):** the key is sourced from
  `--llm-api-key-env <VAR>`, carried as `SecretString`; never `println!`ed.

No new security surface beyond the plan's threat model (loopback listener + a
user-named outbound LLM endpoint, both already modeled).

## Known Stubs

None. `team dev` is fully wired: the default transcript runs end-to-end offline,
`--serve` hosts over HTTP, and `--llm` swaps in a real source. `grep` confirms the
`bail!("team dev: implemented in plan 110-04")` stub is gone.

## Self-Check: PASSED

- `cargo-pmcp/src/commands/team/dev.rs` — FOUND (stub removed: 0 matches).
- `cargo-pmcp/tests/team_dev.rs` — FOUND.
- Commit `bf50b8f3` (test) — present in git history.
- Commit `05b26983` (feat) — present in git history.
