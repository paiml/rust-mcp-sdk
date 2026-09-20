---
phase: 85-shape-a-pure-config-binary-reference-parity
plan: 05
subsystem: shape-a-binary
tags: [pmcp-sql-server, shape-a, assemble, streamable-http, code-mode, resources, prompts, lazy-startup, ref-parity]

# Dependency graph
requires:
  - phase: 85-02
    provides: "LOCKED try_code_mode_from_config_with_connector (validate_code + execute_code); assemble_code_mode_prompt_with_schema; # Database Schema header"
  - phase: 85-04
    provides: "cli::Args { config, schema, http }; dispatch(cfg) -> Arc<dyn SqlConnector> + DispatchError; offline-safe Athena arm"
  - phase: 85-03
    provides: "pmcp-sql-server crate scaffold (lib/main split, placeholder run()); vendored Chinook fixtures (chinook.db, chinook.ddl, reference-config.toml)"
  - phase: 85-01
    provides: "${VAR} token_secret expansion (CODE_MODE_SECRET resolved at code-mode wiring time)"
  - phase: 84-sql-connectors
    provides: "SqlConnector::dialect(); SqliteConnector; offline-safe connector constructors"
provides:
  - "assemble::build_server(cfg, connector, schema_ddl) -> pmcp::Server (tools + code-mode + all configured resources/prompts, schema merged)"
  - "assemble::merge_schema_resource(cfg, schema_ddl) -> Vec<ResourceConfig> (preserves ALL resources, overrides ONLY the /schema URI content)"
  - "lib::run(args) -> Result<(), RunError> (parse -> dispatch -> assemble -> serve over streamable HTTP)"
  - "lib::serve(server, addr) -> (bound_addr, JoinHandle) (Phase 56 adapter; testable non-blocking serve)"
  - "SC-1 lazy-startup proof (athena configs serve tools with no creds, timeout-guarded)"
  - "SC-2 superset-parse proof (all four reference configs parse + dispatch the right dialect)"
affects: [85-06, 86-shapes-bcd]

# Tech tracking
tech-stack:
  added: [url]
  patterns:
    - "merge_schema_resource clones cfg.resources and overrides ONLY the /schema-suffixed URI content with a header-prefixed --schema DDL — every other configured resource passes through verbatim (REVIEW FIX #2)"
    - "Prompts built via StaticPromptHandler::from_configs against the MERGED resource handler so include_resources resolution sees the --schema content (REVIEW FIX #3); not a standalone generated body"
    - "Code-mode via the LOCKED try_code_mode_from_config_with_connector (REVIEW FIX #4) — registers validate_code + execute_code, not the connectorless no-tool path"
    - "serve(server, addr) returns (bound_addr, handle) so the HTTP smoke test drives the server without run() blocking; run() awaits the handle for the process lifetime"
    - "SC-1 wraps dispatch + build_server in tokio::time::timeout so an accidental network/credential wait FAILS fast instead of hanging (D-09 lazy startup proof)"

key-files:
  created:
    - crates/pmcp-sql-server/src/assemble.rs
    - crates/pmcp-sql-server/tests/assemble.rs
    - crates/pmcp-sql-server/tests/superset_parse.rs
    - crates/pmcp-sql-server/tests/http_lazy_startup.rs
  modified:
    - crates/pmcp-sql-server/src/lib.rs
    - crates/pmcp-sql-server/src/main.rs
    - crates/pmcp-sql-server/Cargo.toml

decisions:
  - "merge_schema_resource matches the schema resource by /schema URI suffix (not an exact docs://chinook/schema literal) so the merge is robust across backends whose schema namespace differs (docs://open-images/schema etc.). A defensive append path adds a schema resource when none is configured."
  - "build_server is synchronous (not async): all its toolkit steps are sync; only dispatch (connector construction) and serve (bind) are async. Keeps the assembly seam easy to unit-test and the cog low."
  - "serve uses StreamableHttpServerConfig::default() (stateful, AllowedOrigins::localhost()) NOT stateless() — the loopback default matches --http 127.0.0.1 and applies the DnsRebinding/CORS/SecurityHeaders layers (T-85-05-01); stateless()'s AllowedOrigins::any() is for proxy-fronted Lambda, not the local binary."
  - "url = 2.5 added as a dev-dependency (Rule 3 blocking): the HTTP smoke test's StreamableHttpTransportConfig.url field is a url::Url, mirroring tests/streamable_http_integration.rs; url was not previously a dependency of pmcp-sql-server."
  - "All three non-SQLite reference fixtures are ATHENA (open-images, imdb, msr-vtt) — the plan's read_first mention of 'imdb (mysql)' is inaccurate; verified on disk. SC-1 therefore exercises two athena configs; the mysql/postgres dispatch arms remain covered by Plan 04's dispatch tests."

requirements-completed: [SHAP-A-01, REF-01]

# Metrics
duration: 22min
completed: 2026-05-27
tasks: 2
files: 7
---

# Phase 85 Plan 05: Shape A Server Assembly + Streamable-HTTP Serving Summary

**The Shape A binary now stands up a live MCP server from config + schema alone: `lib::run` parses the config + `--schema`, dispatches the connector for `[database] type`, assembles a `pmcp::Server` (curated `[[tools]]` + the LOCKED connector-aware `validate_code`/`execute_code` + ALL configured resources with the schema content merged + the configured `start_code_mode` prompt), and serves it over streamable HTTP via the Phase 56 adapter — with SC-1 proving athena configs serve tools lazily with no creds under a timeout guard and SC-2 proving all four reference configs parse + dispatch the right connector.**

## Performance

- **Duration:** ~22 min
- **Tasks:** 2
- **Files created/modified:** 7 (4 created, 3 modified)

## Accomplishments

- **`assemble::build_server` (Task 1):** the heart of "no Rust required". Chains `Server::builder().name(...).version(...).try_tools_from_config_with_connector(cfg, connector)?.try_code_mode_from_config_with_connector(cfg, connector)?` (the LOCKED Plan 02 API — REVIEW FIX #4), registers ALL configured prompts resolved against the merged resources, and attaches the merged resource handler. Helpers (`merge_schema_resource`, `build_resource_handler`, `prompt_configs`, `register_prompts`) keep each unit cog ≤ 25 (PMAT: 0 violations).
- **REVIEW FIX #2 (resource preservation):** `merge_schema_resource` clones every configured `[[resources]]` entry and replaces ONLY the `/schema`-suffixed URI's content with the `# Database Schema\n\n`-prefixed `--schema` DDL (header byte-identical to the Plan 02 prompt seam). `docs://chinook/examples` and `code-mode://learnings` pass through with their configured content. The assemble test asserts all three resolve with the correct content.
- **REVIEW FIX #3 (prompt preservation):** prompts come from `cfg.prompts` via `StaticPromptHandler::from_configs(&prompt_configs, &merged_resources)`, so `start_code_mode`'s `include_resources` resolve against the MERGED handler (the schema portion reflects `--schema`). The test calls the registered prompt's `handle` and asserts a non-empty body carrying the merged schema content.
- **`lib::run` + `serve` (Task 2):** the full pipeline — `load_config_and_schema` → `dispatch` → `build_server` → `serve` (`StreamableHttpServer::with_config(addr, Arc<Mutex<Server>>, default()).start()`) → await the handle. `serve(server, addr) -> (bound_addr, handle)` is public + non-blocking so the HTTP smoke drives the server directly. `RunError` carries credential-free Display. `main.rs` is a 3-line `#[tokio::main]` shim (`Args::parse()` → `run`).
- **SC-1 (`http_lazy_startup.rs`):** open-images + imdb (both athena) `dispatch + build_server` complete WITH NO creds env (explicitly cleared) inside a 10s `tokio::time::timeout` — a hang would trip the timeout and fail. Plus an HTTP smoke: `serve` on `127.0.0.1:0`, send an MCP `initialize` via `StreamableHttpTransport`, assert the response echoes the request id, abort the handle.
- **SC-2 (`superset_parse.rs`):** all four reference configs parse via `from_toml_strict_validated` and `dispatch` resolves the correct dialect — chinook → `Dialect::Sqlite`, open-images/imdb/msr-vtt → `Dialect::Athena` — under default features so all backends compile (REF-01 at the binary boundary).

## Task Commits

1. **Task 1: assemble.rs — build pmcp::Server preserving all configured resources + prompts** — `2727a8a6` (feat)
2. **Task 2: fill lib::run with streamable-HTTP serving + SC-1/SC-2 tests** — `d6795c3e` (feat)

_TDD note: each task co-located tests + implementation in the same touched files (behavior written first, then implementation, verified green before commit)._

## Files Created/Modified

- `crates/pmcp-sql-server/src/assemble.rs` — NEW. `build_server`, `merge_schema_resource` (pub), `build_resource_handler`/`prompt_configs`/`register_prompts` (private helpers), `AssembleError`, `SCHEMA_HEADER`/`SCHEMA_URI_SUFFIX` consts. 2 unit tests.
- `crates/pmcp-sql-server/tests/assemble.rs` — NEW. 4 integration tests (curated+code-mode tools, all-three-resources-merged, start_code_mode prompt resolved, no-code-mode no-op) against the data-bearing Chinook fixture.
- `crates/pmcp-sql-server/src/lib.rs` — replaced the Plan 03 placeholder `RunConfig`/`run` with the real `run(args)`, `serve`, `load_config_and_schema`, `RunError`. Re-exports `build_server`, `merge_schema_resource`, `AssembleError`.
- `crates/pmcp-sql-server/src/main.rs` — now a thin `#[tokio::main]` shim: `Args::parse()` → `run(args)`.
- `crates/pmcp-sql-server/tests/superset_parse.rs` — NEW. SC-2: 4 parse+dispatch-dialect tests across all four reference configs.
- `crates/pmcp-sql-server/tests/http_lazy_startup.rs` — NEW. SC-1: 2 athena lazy-startup tests (timeout-guarded, no creds) + 1 HTTP initialize smoke.
- `crates/pmcp-sql-server/Cargo.toml` — added `url = "2.5"` dev-dep.

## Decisions Made

- **Schema match by `/schema` URI suffix** (not an exact literal) so `merge_schema_resource` works across backends with different schema namespaces; a defensive append covers configs with no schema resource.
- **`build_server` synchronous:** only `dispatch` (connector construction) and `serve` (bind) are async; the assembly itself is sync, which keeps the seam trivially unit-testable.
- **`StreamableHttpServerConfig::default()` (stateful, localhost CORS)** over `stateless()` — the loopback default matches `--http 127.0.0.1` and applies the SDK's DnsRebinding/CORS/SecurityHeaders layers (T-85-05-01). `stateless()`'s `AllowedOrigins::any()` is for proxy-fronted Lambda, not a local binary.
- **`url` dev-dep (Rule 3 blocking):** the HTTP smoke's transport config requires `url::Url`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `url = "2.5"` dev-dependency to `pmcp-sql-server`**
- **Found during:** Task 2 (the HTTP smoke test failed to compile: `unresolved import url`)
- **Issue:** `StreamableHttpTransportConfig.url` is a `url::Url`, but `url` was not a (dev-)dependency of `pmcp-sql-server`.
- **Fix:** added `url = "2.5"` (matching the workspace pin) to `[dev-dependencies]` with a `# Why` comment.
- **Files modified:** `crates/pmcp-sql-server/Cargo.toml`
- **Commit:** `d6795c3e`

**2. [Plan inaccuracy — documented, not a code change] All three non-SQLite fixtures are Athena, not "imdb (mysql)"**
- **Found during:** Task 2 fixture inspection
- **Issue:** The plan's `<read_first>`/`<behavior>` describe "imdb (mysql) and open-images (athena)". On disk, imdb-config.toml, open-images-config.toml, and msr-vtt-config.toml are ALL `type = "athena"`. There is no mysql reference fixture.
- **Resolution:** SC-1 exercises the two athena configs (open-images + imdb); SC-2 covers all four (one sqlite + three athena). The mysql/postgres dispatch arms remain covered by Plan 04's `tests/dispatch.rs`. No fixture was invented or renamed (REF-01 superset invariant preserved).

---

**Total deviations:** 1 auto-fixed (blocking dev-dep), 1 documented plan/fixture mismatch (no code impact beyond test backend selection).

## Issues Encountered

- The `pmcp::Server` has no public accessor to read its resource handler by URI, so the "all three resources" assertion builds the merged `StaticResourceHandler` directly via the public `merge_schema_resource` + `StaticResourceHandler::from_configs` (exactly what `build_server` does internally) and asserts via `handler.get(uri)`. The prompt assertion uses the public `server.get_prompt(name)` + `handle`.

## Deferred Issues

Out-of-scope pre-existing clippy lint surfaced under the local rust-1.95.0 toolchain (NOT introduced by this plan; same class already logged in STATE.md / `deferred-items.md`):

- `crates/pmcp-server-toolkit/src/code_mode.rs:460-461` — `field_reassign_with_default` in `build_cm_config` (Phase 83 code, untouched here). It is the only diagnostic `cargo clippy -p pmcp-sql-server` emits, and it points at the toolkit dependency, not this crate. Per the SCOPE BOUNDARY rule it is NOT fixed. All FIVE of this plan's touched source/test files are fmt-clean and produce zero clippy warnings.

Also reverted an unrelated whitespace-only `cargo fmt` change to `tests/schema_fixture.rs` (a Plan 03 file the workspace-wide formatter touched) to keep the Task 2 commit scoped.

## Verification

```
cargo test -p pmcp-sql-server --no-default-features --features sqlite --test assemble -- --test-threads=1
```
→ 4 passed (Task 1: curated+code-mode tools, three resources merged, prompt resolved, no-code-mode no-op).

```
cargo test -p pmcp-sql-server --test http_lazy_startup --test superset_parse -- --test-threads=1
```
→ 7 passed (SC-1: 2 athena lazy-startup + 1 HTTP smoke; SC-2: 4 parse+dispatch-dialect).

```
cargo test -p pmcp-sql-server -- --test-threads=1
```
→ 33 passed across all suites (7 lib + 4 cli/assemble unit + 8 dispatch + 3 http + 6 schema_fixture + 4 superset + 1 doctest).

PMAT: `analyze complexity --max-cognitive 25` reports 0 violations in `pmcp-sql-server`.

Source assertions (acceptance criteria):
- `grep StreamableHttpServer::with_config crates/pmcp-sql-server/src/lib.rs` → match
- `grep timeout crates/pmcp-sql-server/tests/http_lazy_startup.rs` → 6 matches
- `grep try_tools_from_config_with_connector | try_code_mode_from_config_with_connector | merge_schema_resource | StaticPromptHandler::from_configs crates/pmcp-sql-server/src/assemble.rs` → all present (15 combined)
- assemble.rs toolkit import is a SINGLE crate-root `use pmcp_server_toolkit::{...}` line (D-15)

## Known Stubs

None. The append-a-schema-resource branch in `merge_schema_resource` is a defensive fallback (the reference config always declares the schema resource, so the override path is primary), not a stub.

## Threat Surface

No new threat surface beyond the plan's `<threat_model>`:
- **T-85-05-01 (spoofing, local HTTP):** mitigated — `serve` uses `StreamableHttpServer::start`, which applies the DnsRebinding + CORS + SecurityHeaders layers with `AllowedOrigins::localhost()` (default config). No hand-rolled axum.
- **T-85-05-02 (info disclosure, merged schema):** mitigated — only the schema resource's content is replaced with the admin-redacted `--schema` file; the other resources pass through unchanged; no live `schema_text()` introspection path in assembly.
- **T-85-05-03 (info disclosure, lazy startup):** mitigated — SC-1 dispatches the athena configs with creds explicitly cleared, timeout-guarded; build completes offline.
- **T-85-05-04 (tampering, request size):** accepted — `StreamableHttpServerConfig::default()` carries `DEFAULT_MAX_REQUEST_BYTES` (SDK-owned bound).

## Next Phase Readiness

- **Plan 85-06 (parity replay):** the binary now serves a live MCP server from the Chinook config + `chinook.ddl` over streamable HTTP. Plan 06 can spawn `run`/`serve` against the data-bearing `chinook.db` fixture and replay the 29 `generated.yaml` scenarios (the HTTP smoke here proves `initialize` works end-to-end; the full tools/resources/prompts replay is Plan 06's job). `validate_code` + `execute_code` + all three resources + the `start_code_mode` prompt are all present on the assembled server.
- No blockers. The deferred toolkit clippy lint persists (CI toolchain mismatch) but does not affect this crate's files.

## Self-Check: PASSED

- Created/modified files verified on disk: `src/assemble.rs`, `src/lib.rs`, `src/main.rs`, `tests/assemble.rs`, `tests/superset_parse.rs`, `tests/http_lazy_startup.rs`, `Cargo.toml` (all FOUND).
- Commits verified in git log: `2727a8a6` (Task 1), `d6795c3e` (Task 2) — both present.
- Source assertions verified: `StreamableHttpServer::with_config`, `timeout`, `try_tools_from_config_with_connector`, `try_code_mode_from_config_with_connector`, `merge_schema_resource`, `StaticPromptHandler::from_configs` all match.

---
*Phase: 85-shape-a-pure-config-binary-reference-parity*
*Completed: 2026-05-27*
