---
phase: 85-shape-a-pure-config-binary-reference-parity
plan: 06
subsystem: shape-a-binary
tags: [pmcp-sql-server, ref-parity, code-mode-policy, mcp-tester, fuzz, doctest, shape-c, publish-order]

# Dependency graph
requires:
  - phase: 85-05
    provides: "lib::run / serve (parse --config/--schema → dispatch → build_server → StreamableHttpServer); data-bearing chinook.db + chinook.ddl + reference-config.toml + generated.yaml fixtures"
  - phase: 85-02
    provides: "try_code_mode_from_config_with_connector (validate_code + execute_code); NoopPolicyEvaluator static-policy enforcement"
  - phase: 84-sql-connectors
    provides: "SqliteConnector (file + in-memory); SynthesizedToolHandler param binding seam"
provides:
  - "lib::run_serving(&Args) -> (bound_addr, JoinHandle) — the testable seam exposing the FULL real binary path (run delegates to it then awaits)"
  - "tests/parity_chinook.rs — REF-02/SC-3/SC-4 binding assertion: spawn through the real --config --schema path, replay all 29 generated.yaml scenarios, assert result.success"
  - "examples/sql_server_min.rs — Shape C ALWAYS-matrix runnable smoke example"
  - "seed-chinook-superset.toml — config-parser fuzz seed for file_path/is_reference/[shared_policy_store] mixing ${VAR}/env:VAR"
  - "CLAUDE.md publish-order slot for pmcp-sql-server (item 9)"
affects: [86-shapes-bcd, 89-docs]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "run_serving extracted from run as a testable seam returning (bound_addr, handle); run() = run_serving().await + handle.await — the binary path is byte-for-byte the same path the parity test drives (NO build_server connector injection)"
    - "parity harness writes a temp config.toml (reference-config.toml with [database] file_path string-replaced to a temp copy of the data-bearing chinook.db) + temp chinook.ddl --schema, then invokes run_serving via programmatic clap-free Args — exercising ServerConfig::from_toml_strict_validated + dispatch (reading file_path) + build_server + StreamableHttpServer"
    - "mcp-tester replay: ServerTester::new(http url, force_transport=http) → test_initialize() readiness poll (sets up the reusable pmcp client the executor needs) → ScenarioExecutor::execute(TestScenario::from_file(generated.yaml)) → assert result.success"
    - "validate_code policy rejection now surfaces as a tool Err (isError:true) — the production reference observable the generated.yaml failure assertions verify"
    - "declared [[tools.parameters]] default applied when the caller omits the arg — fixes unbound-NULL :limit/:offset binding"

key-files:
  created:
    - crates/pmcp-sql-server/tests/parity_chinook.rs
    - crates/pmcp-sql-server/examples/sql_server_min.rs
    - crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-chinook-superset.toml
  modified:
    - crates/pmcp-sql-server/src/lib.rs
    - crates/pmcp-server-toolkit/src/tools.rs
    - crates/pmcp-server-toolkit/src/code_mode.rs
    - crates/pmcp-server-toolkit/tests/code_mode_tools.rs
    - CLAUDE.md

decisions:
  - "run_serving seam (not refactor-run-to-return-addr): run() keeps its blocking long-running shape; run_serving exposes (bound_addr, handle) so the parity test obtains the ephemeral port + aborts the task. Both go through the IDENTICAL load_config_and_schema → dispatch → build_server → serve pipeline — the test is the real binary path, not a divergent fork."
  - "Temp config via string-replace of the single file_path line (not parse→reserialize): keeps the ~20 KB embedded markdown resource content byte-identical to the production reference; only [database] file_path → temp chinook.db copy changes."
  - "Readiness via test_initialize() backoff loop (D-disc poll-over-sleep): the executor's HTTP path needs the reusable pmcp_client that test_initialize installs; the loop doubles as the bind-readiness gate."
  - "Assert result.success (not a hardcoded count): generated.yaml has 29 named scenarios (verified grep -c '^- name:'); asserting success is count-stable."
  - "REF-02 'open-images' literal is satisfied by the SQLite Chinook reference that OWNS generated.yaml (D-01 approved scope reading) — the data-bearing, offline-runnable reference vendored in Plan 03, not a gap."

requirements-completed: [REF-02, SHAP-A-01]

# Metrics
duration: 38min
completed: 2026-05-27
tasks: 2
files: 8
---

# Phase 85 Plan 06: Reference-Parity Replay Through the Real Binary Path Summary

**The Shape A toolkit lift is proven: the `pmcp-sql-server` binary, exercised through the REAL `--config --schema` path (programmatic `Args` → `run_serving` → `ServerConfig::from_toml_strict_validated` → `dispatch` reading `file_path` → `build_server` → `StreamableHttpServer` — no connector injection), reproduces the production Chinook reference server and passes all 29 `generated.yaml` scenarios replayed via the `mcp-tester` library — curated tools on REAL data, `validate_code`×8 + `execute_code` with code-mode policy REJECTING writes/DDL/no-LIMIT/forged-tokens, the `start_code_mode` prompt, and all 3 resources — closing REF-02 (result parity / SC-4), the SC-3 code-mode policy-parity proof, and the ALWAYS matrix (fuzz + doctest + integration + runnable example).**

## Performance

- **Duration:** ~38 min
- **Tasks:** 2
- **Files created/modified:** 8 (3 created, 5 modified)

## Accomplishments

- **REF-02/SC-3/SC-4 binding assertion (`tests/parity_chinook.rs`, Task 1):** spawns the Shape A repro through the real binary path and replays the full 29-scenario production contract, asserting `result.success`. The data-value assertions (`search_tracks` → "Rock"/"AC/DC", `get_album_tracks` → "For Those About To Rock (We Salute You)", `list_artists` → "AC/DC") pass ONLY because the fixture is data-bearing (REVIEW FIX #1 proof end-to-end). All 3 resources resolve (REVIEW FIX #2) and `start_code_mode` resolves (REVIEW FIX #3). The DELETE/DDL/DROP/no-LIMIT `validate_code` + invalid-token `execute_code` `failure` assertions confirm the static `[code_mode]` policy rejects writes/DDL/forged tokens through HTTP (SC-3, threat T-85-02-02 black-box proof).
- **`run_serving` testable seam (`lib.rs`, Task 1):** extracted the full pipeline (`load_config_and_schema` → `dispatch` → `build_server` → `serve`) into `run_serving(&Args) -> (SocketAddr, JoinHandle)`. `run()` now = `run_serving().await` + `handle.await`, so the binary's path is the *exact* path the parity test drives — satisfying Codex HIGH #5 ("must exercise the REAL binary, not an injected connector").
- **Shape C smoke example (`examples/sql_server_min.rs`, Task 2):** a ≤15-line `main` building a `pmcp::Server` from an inline config + in-memory `SqliteConnector` via `build_server`. The ALWAYS-matrix runnable example for the binary crate (NOT the Phase 86 Shape C library contract).
- **Doctests (`lib.rs`, Task 2):** `rust,no_run` doctests on `run`, `run_serving`, `serve` (`cargo test --doc` → 4 passing, incl. the existing `Args` doctest).
- **CLAUDE.md publish order (Task 2):** `pmcp-sql-server` slotted as item 9 (after the per-backend connectors; items renumbered 10–12) with the no-inter-dep-with-mcp-tester note.
- **Fuzz seed (`seed-chinook-superset.toml`, Task 2):** exercises `[database] file_path`, `[server] is_reference`, `[shared_policy_store]`, and mixes `${VAR}` (token_secret) + `env:VAR` (url) indirection forms (ALWAYS fuzz, T-85-06-01). Parses green under the `reference_configs` seed-parse smoke test.

## Task Commits

1. **Task 1: REF-02 parity replay through the real --config --schema binary path** — `27742942` (feat)
2. **Task 2: Shape C smoke example + doctests + publish-order + fuzz seed (ALWAYS closeout)** — `e217f47f` (feat)

_TDD note (Task 1): the test (RED) was authored first against the `run_serving` seam; running it surfaced two real binary-path defects (below), which were fixed (GREEN) and re-verified before commit._

## Files Created/Modified

- `crates/pmcp-sql-server/tests/parity_chinook.rs` — NEW. The REF-02/SC-3/SC-4 replay (temp config + temp schema → `run_serving` → mcp-tester replay of all 29 scenarios → `assert!(result.success)`).
- `crates/pmcp-sql-server/examples/sql_server_min.rs` — NEW. Shape C runnable smoke example.
- `crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-chinook-superset.toml` — NEW. Superset-fields fuzz seed.
- `crates/pmcp-sql-server/src/lib.rs` — extracted `run_serving` seam; added doctests to `run`/`run_serving`/`serve`.
- `crates/pmcp-server-toolkit/src/tools.rs` — `extract_named_params` applies declared parameter defaults (Rule 1 bug fix).
- `crates/pmcp-server-toolkit/src/code_mode.rs` — `ValidateCodeHandler` surfaces policy rejections as a tool `Err` (Rule 1 bug fix).
- `crates/pmcp-server-toolkit/tests/code_mode_tools.rs` — updated the two rejection tests to the corrected error-on-rejection contract.
- `CLAUDE.md` — publish-order slot for `pmcp-sql-server`.

## Decisions Made

- **`run_serving` seam over forking the pipeline:** the parity test drives the SAME `load_config_and_schema → dispatch → build_server → serve` path the binary runs; `run()` just adds the trailing `handle.await`. No divergent test-only assembly.
- **Temp config via single-line string-replace:** preserves the production reference byte-for-byte except `[database] file_path` → temp `chinook.db` copy.
- **Readiness via `test_initialize()` backoff loop:** doubles as the executor's required pmcp-client setup and the bind-readiness gate (poll-over-sleep).
- **Assert `result.success`, not a hardcoded count:** 29 named scenarios verified; success is count-stable.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `search_tracks` / `list_artists` failed with SQLite "datatype mismatch" — declared parameter defaults were never applied**
- **Found during:** Task 1 (the data-value scenarios that omit `limit`/`offset` failed with `connector error: query error: datatype mismatch`).
- **Issue:** `extract_named_params` (toolkit `tools.rs`) only forwarded parameters PRESENT in the caller's args. When a caller omitted an optional parameter that declares a `default` (e.g. `search_tracks {query:"Rock"}` with no `limit`), the `:limit`/`:offset` placeholder bound as unbound `NULL`; SQLite rejects `LIMIT NULL` with "datatype mismatch". The declared `default = 20` was synthesized into the input schema but never applied at bind time.
- **Fix:** `extract_named_params` now falls back to the declared `default` (`toml::Value` → `serde_json::Value`) when the arg is absent.
- **Files modified:** `crates/pmcp-server-toolkit/src/tools.rs`
- **Commit:** `27742942`

**2. [Rule 1 - Bug] `validate_code` policy rejections returned a silent success instead of a tool error**
- **Found during:** Task 1 (the DELETE/DDL/DROP/no-LIMIT `failure` assertions failed — the rejections came back as MCP successes carrying `valid:false`).
- **Issue:** `ValidateCodeHandler::handle` (toolkit `code_mode.rs`) discarded the `is_error` flag from `ValidationResponse::to_json_response()` and always returned `Ok(json)`. A policy rejection (`allow_deletes=false`, `allow_ddl=false`, `require_limit`) therefore surfaced as a `tools/call` SUCCESS, not an error — so the reference `failure` assertions (which check for an error response) never matched. The production reference observable is an error.
- **Fix:** when `to_json_response()` reports `is_error == true`, the handler now returns `Err(pmcp::Error::Internal(<rejection-json>))` so the `tools/call` result has `isError:true`; the rejection JSON (valid:false + violation) is carried in the error message.
- **Files modified:** `crates/pmcp-server-toolkit/src/code_mode.rs`; the two Plan 85-02 rejection tests in `crates/pmcp-server-toolkit/tests/code_mode_tools.rs` were updated to the corrected contract (they had encoded the old silent-success behavior).
- **Commit:** `27742942`

### Documented (no code impact)

**3. [Plan wording — count]** `generated.yaml` has 29 named scenarios (verified), as the plan's REVIEW FIX already noted. The test asserts `result.success` (count-stable) rather than a hardcoded number.

---

**Total deviations:** 2 auto-fixed Rule 1 binary-path bugs (both essential for the parity replay; both are corrections to the *production reference path*, not test workarounds), 1 documented count note.

## Issues Encountered

- The mcp-tester library exposes readiness via `ServerTester::test_initialize()` (returns a `TestResult`), not the `initialize()` the plan's interface sketch named. The harness polls `test_initialize()` with backoff — which also installs the reusable `pmcp_client` the `ScenarioExecutor` HTTP path requires.

## Deferred Issues

- **Pre-existing pedantic clippy lint (NOT introduced here):** `crates/pmcp-server-toolkit/src/code_mode.rs:471-472` — `field_reassign_with_default` in `build_cm_config` (Phase 83 code, untouched by this plan). It is the single diagnostic `cargo clippy -p pmcp-sql-server` emits and points at the toolkit dependency, not this plan's edits. Per the SCOPE BOUNDARY rule it is NOT fixed; it is the same rust-1.95.0-vs-CI toolchain-mismatch class already logged in STATE.md / `deferred-items.md`.
- **Pre-existing fmt diff (NOT introduced here):** `crates/pmcp-server-toolkit/src/sql/sqlite.rs` carries rust-1.95.0 formatting differences in untouched functions. All FIVE source/test files touched by this plan are fmt-clean (verified per-file with `rustfmt --check`).

## Verification

```
cargo test -p pmcp-sql-server --no-default-features --features sqlite -- --test-threads=1
```
→ 30 passed (7 lib + 4 cli/assemble unit + 8 dispatch + 1 parity + 6 schema_fixture + 4 superset + 4 doctest). The parity replay (`result.success`, all 29 scenarios) is green via the real `--config --schema` binary path.

```
cargo test -p pmcp-sql-server --test parity_chinook -- --test-threads=1   # default features (all 4 connectors)
```
→ 1 passed (parity also green under the full default-feature build, REF-01 at the binary boundary).

```
cargo run -p pmcp-sql-server --example sql_server_min --no-default-features --features sqlite
```
→ exits 0, prints `pmcp-sql-server example: built 'SQL Server Min Demo' with 1 curated tool(s) from config`.

```
cargo test -p pmcp-sql-server --doc --no-default-features --features sqlite
```
→ 4 passed.

```
cargo test -p pmcp-server-toolkit --features "code-mode sqlite" -- --test-threads=1
```
→ 187 passed (incl. the updated `code_mode_tools` rejection contract + the `reference_configs` seed-parse smoke test picking up `seed-chinook-superset.toml`).

Source assertions (acceptance criteria):
- `grep -c "Args" tests/parity_chinook.rs` → matches (programmatic `Args`); `grep "result.success"` → matches; `grep "generated.yaml"` → matches; `grep "ScenarioExecutor"` → matches.
- No `build_server(.., <injected connector>, ..)` call in the test body — `build_server` appears ONLY in doc comments; the assembly goes through `run_serving` (the real binary path).
- `grep "pmcp-sql-server" CLAUDE.md` → matches in the publish-order section (item 9).
- `grep -c "^- name:" tests/fixtures/generated.yaml` → 29.

## Known Stubs

None. The `let _ = server;` line in the example is a deliberate "a real binary would serve here" annotation, not a stub — the server is fully built from config.

## Threat Surface

No new threat surface beyond the plan's `<threat_model>`:
- **T-85-02-02 (elevation of privilege, re-verified):** mitigated — the `generated.yaml` DELETE/DDL/no-LIMIT `validate_code` + invalid-token `execute_code` `failure` assertions confirm the static policy rejects writes/DDL/forged tokens end-to-end through HTTP via the real binary path (black-box proof on top of Plan 02's white-box unit tests). The code-mode bug fix (deviation #2) is what makes this rejection observable as an MCP error.
- **T-85-06-01 (DoS, new config fields under fuzz):** mitigated — `seed-chinook-superset.toml` extends the config-parser fuzz target so `file_path`/`is_reference`/`[shared_policy_store]` + `${VAR}`/`env:VAR` parsing is covered by the no-panic invariant.
- **T-85-06-02 (spoofing, parity harness HTTP):** accept — test-only loopback `127.0.0.1:0` with `force_transport "http"`; no production exposure.

## Self-Check: PASSED

- Created files verified on disk: `tests/parity_chinook.rs`, `examples/sql_server_min.rs`, `fuzz/corpus/pmcp_server_toolkit_config_parser/seed-chinook-superset.toml` (all FOUND).
- Commits verified in git log: `27742942` (Task 1), `e217f47f` (Task 2) — both present.
- Source assertions verified: `Args`, `result.success`, `generated.yaml`, `ScenarioExecutor` all match; `pmcp-sql-server` present in CLAUDE.md publish order; 29 scenarios.

---
*Phase: 85-shape-a-pure-config-binary-reference-parity*
*Completed: 2026-05-27*
