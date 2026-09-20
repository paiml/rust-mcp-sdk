---
phase: 86
reviewers: [gemini, codex]
reviewed_at: 2026-05-27T05:33:05Z
plans_reviewed: [86-01-PLAN.md, 86-02-PLAN.md, 86-03-PLAN.md, 86-04-PLAN.md, 86-05-PLAN.md, 86-06-PLAN.md]
---

# Cross-AI Plan Review — Phase 86

## Gemini Review

# Phase 86 Plan Review: Shapes B/C/D — Scaffold, Library Example, Deploy

The implementation plans for Phase 86 are exceptionally well-researched and strategically sequenced. By prioritizing the "Shape C" (library-use) example in Wave 2, the plans establish a verified wiring "blueprint" that is then byte-identically emitted by the "Shape B" (scaffold) emitter in Wave 3. The use of automated line-count assertions to enforce the "≤15-line" requirement is a standout design choice that ensures non-functional goals are validated with the same rigor as functional ones.

## Strengths
- **Blueprint-First Sequencing:** Implementing the library example (Shape C) before the scaffold (Shape B) is a classic "build what you emit" strategy that eliminates speculative template work.
- **Automated Ergonomics Enforcement:** The inclusion of an integration test that specifically asserts the line count of `main.rs` is a high-signal validation pattern that prevents "ergonomics drift" during implementation.
- **Robust Dependency Resolution:** The use of `[patch.crates-io]` in the tempdir scaffold tests (Plan 04) effectively solves the "unpublished toolkit" blocker identified in Research Pitfall §1.
- **Surgical Deploy Integration:** The detection-based approach for Shape D (Plan 05) honors the "no breaking changes" (D-10) constraint perfectly by avoiding modifications to the `TargetEntry` enum.
- **Authentic Verification:** Env-gating the real deploy test (Plan 06) balances the need for authentic end-to-end proof with the reality of CI environment limitations.

## Concerns

### 1. Plan 01/02 Mismatch: `Arc<dyn SqlConnector>` vs Inherent Method (MEDIUM)
Plan 01 adds `execute_batch` as an inherent method on `SqliteConnector` (to avoid touching the locked `SqlConnector` trait). However, Plan 02 (Task 1) and the Research Code Example 1 attempt to call `execute_batch` on an object typed as `Arc<dyn SqlConnector>`. 
*   **Impact:** The code in Plan 02 will fail to compile because `execute_batch` is not on the trait.
*   **Resolution:** The wiring in `main.rs` must call `execute_batch` on the concrete `SqliteConnector` instance *before* it is wrapped in an `Arc<dyn SqlConnector>` for the builder, or a free helper function in the toolkit should be used.

### 2. SQLite Asset Path Rigidity on Lambda (MEDIUM)
Plan 05 mentions that the scaffold will "comment" the `/var/task/assets/` path in the emitted `deploy.toml`/`config.toml`. 
*   **Impact:** Users deploying for the first time might miss the comment, leading to a "DB not found" error on Lambda because the local relative path (`demo.db`) won't work in the Lambda environment.
*   **Resolution:** Consider emitting the `file_path` with an environment variable expansion (e.g., `file_path = "${DB_PATH:demo.db}"`) so the scaffold works locally out of the box, and the deploy process (or the user) can override it via a `DB_PATH` environment variable.

### 3. Build Performance in CI (LOW)
Plan 04 shells out to a real `cargo run` in a tempdir. 
*   **Impact:** Compiling the toolkit and the SDK from scratch in a fresh tempdir during every test run can be quite slow and may hit CI timeouts if not cached. 
*   **Resolution:** While acceptable for the "verified end-to-end" goal, ensure the test is included in the `--test-threads=1` group to prevent resource contention.

## Suggestions
- **Refine the Wiring Blueprint:** Adjust the `main.rs` wiring in Plan 02 to call `conn.execute_batch` while `conn` is still a concrete `SqliteConnector` to ensure it compiles without needing a trait change.
- **Unified Serve Helper:** Since the `StreamableHttpServer` boilerplate is repeated in both the example and the scaffold (and inlines the `serve` body), consider adding a thin `pmcp_server_toolkit::serve` helper in Plan 01. This would reduce the `main.rs` body by 2-3 lines and make the budget even easier to hit.
- **Scaffold Dependency Patching:** In Plan 04, ensure the `[patch.crates-io]` logic handles transitive dependencies (like `pmcp-code-mode`) if they aren't yet on crates.io, or the `cargo run` will fail.

## Risk Assessment: LOW
The overall risk is low. The research phase has empirically verified the ≤15-line budget and identified the critical "inline-secret" and "batch-execute" pitfalls. The plans are highly surgical and stay strictly within the phase boundaries. Addressing the `Arc<dyn SqlConnector>` type mismatch before execution will ensure a smooth Wave 2.

**Verdict:** The plans are ready for execution, provided the type mismatch in Plan 02 is corrected in-flight.

---

## Codex Review

**Overall Summary**
The phase is generally well decomposed: Shape C is proven before Shape B emits it, TEST-05 validates the scaffold with a real subprocess, and Shape D is intentionally separated from the gated real deploy proof. The biggest risks are in Shape D and test mechanics. Several plans assume existing deploy/build behavior that the current code does not provide for a single-crate Lambda project, and the asset/config story for Lambda does not yet line up with how the generated `main.rs` reads files.

**86-01**
**Summary:** Solid enabling plan for the SQLite batch bootstrap helper, but it has an ordering hazard around registering an example before the file exists.

**Strengths**
- Keeps `execute_batch` as an inherent `SqliteConnector` method, preserving the 3-method `SqlConnector` trait.
- Correctly uses `spawn_blocking` around `rusqlite::Connection::execute_batch`.
- Good tests: success, multi-table bootstrap, invalid SQL error path, doctest.
- Explicitly de-risks the single-crate deploy question before Shape D.

**Concerns**
- **HIGH:** Adding `[[example]] name = "sql_server_http"` before `examples/sql_server_http.rs` exists may make Cargo reject the manifest. Do not register a non-existent example target in Wave 1 unless a placeholder file is also created.
- **MEDIUM:** The deploy spike is only “recorded in SUMMARY”; if it discovers `find_lambda_package_dir` does not support single-crate projects, later plans need `builder.rs` in scope.
- **LOW:** The doctest needs a clear async wrapper, usually `#[tokio::main]`, and the required feature context should be explicit.

**Suggestions**
- Move example registration to 86-02, or create a tiny placeholder example in 86-01.
- Make the spike output concrete: “single-crate supported yes/no; exact files needed if no.”
- Add an idempotency note for `execute_batch` consumers: schema files should use `CREATE TABLE IF NOT EXISTS` and `INSERT OR IGNORE`.

**Risk Assessment:** **MEDIUM**. The helper itself is low risk, but the Cargo target ordering issue can break the workspace between waves.

**86-02**
**Summary:** Good plan for proving Shape C early, but the process-spawn strategy and line-count interpretation need tightening.

**Strengths**
- Correctly avoids depending on `pmcp-sql-server::serve`.
- Uses the toolkit builder chain instead of re-implementing `build_server`.
- Covers `initialize`, `tools/list`, and `tools/call`.
- Validates the inline dev secret behavior that could otherwise fail at runtime.

**Concerns**
- **HIGH:** The example will likely fail on second run unless `schema.sql` is idempotent. A persisted `demo.db` plus plain `CREATE TABLE`/`INSERT` is fragile.
- **HIGH:** Lambda/file asset behavior is not considered here, but this same `main.rs` becomes the scaffold/deploy entrypoint. Reading `schema.sql` from a local fixture path will not work unchanged in Lambda.
- **MEDIUM:** `env!("CARGO_BIN_EXE_...")` is not reliable for examples. Spawning `cargo run --example ...` is more realistic but slow and can contend on Cargo locks.
- **MEDIUM:** “≤15-line body” may not satisfy a strict reading of “≤15 lines of `main.rs`.” The verifier needs the same definition as the plan.
- **LOW:** The config’s `[database].file_path` should match the connector path or the example has two conflicting database sources.

**Suggestions**
- Make the fixture schema idempotent.
- Decide now whether the canonical `main.rs` uses `pmcp::assets` or a small path resolver so the same code works locally and in Lambda.
- Prefer a test helper that spawns `cargo run --example` with a timeout and child-kill guard.
- Define line counting as physical non-empty body lines, and document that imports/attributes are excluded.

**Risk Assessment:** **MEDIUM**. The runtime shape is sound, but idempotency and asset-path behavior can cause real failures downstream.

**86-03**
**Summary:** Strong Shape B scaffold plan with good reuse of Shape C, but it under-specifies crate-name/path validation and inherits the file-path issues from Shape C.

**Strengths**
- Honors the required command literally: `cargo pmcp new --kind sql-server`.
- Keeps the existing workspace path and `sqlite-explorer` template untouched.
- Emits the right four core files and pins both `sqlite` and `code-mode`.
- Scoped README update is appropriately bounded to this phase.

**Concerns**
- **HIGH:** Generated `main.rs` must be truly compatible with both local `cargo run` and later Lambda deploy. If it reads `schema.sql` only from the crate root, Plan 05’s asset bundling may not work.
- **MEDIUM:** Existing name handling allows invalid Cargo package names and possibly path traversal-like names. Reusing the current guard is not a full mitigation.
- **MEDIUM:** End-user `cargo run` depends on unpublished crates becoming available. The plan handles CI later with patches, but the README should avoid implying this works from crates.io before publish.
- **LOW:** Documenting config-only deploy before Plan 05 implements it risks docs drifting from behavior.

**Suggestions**
- Add minimal crate-name validation for `--kind sql-server` output.
- Emit idempotent `schema.sql`.
- Include `deploy.toml` either here or explicitly state it comes in Plan 05, since Shape D depends on scaffold output.
- Add a golden test or source assertion comparing emitted `main.rs` to the example modulo path constants.

**Risk Assessment:** **MEDIUM**. The scaffold is straightforward, but small omissions can make the “runs out of the box” promise brittle.

**86-04**
**Summary:** This is the right kind of TEST-05: tempdir scaffold, real build, real server, real MCP calls. It needs stronger process control and a narrower execution path.

**Strengths**
- Correctly patches unpublished workspace crates for tempdir resolution.
- Uses a real `cargo run` subprocess, which matches the success criterion.
- Exercises both `tools/list` and `tools/call`.
- Uses readiness polling instead of fixed sleeps.

**Concerns**
- **HIGH:** The test must have a robust child-process guard. Any panic before cleanup can leak a server process.
- **MEDIUM:** “Either call `new::execute` in-process OR shell CLI” weakens command-surface coverage. TEST-05 should exercise the real `cargo-pmcp` binary if the command itself is part of the requirement.
- **MEDIUM:** Parsing the bound address from `cargo run` stdout must handle Cargo build output, warnings, and app logs.
- **MEDIUM:** A full tempdir `cargo run` can be slow. It needs a generous timeout and clear failure diagnostics.
- **LOW:** Extra transitive `[patch.crates-io]` entries may not be necessary if path dependencies inside `pmcp-server-toolkit` already point at local crates.

**Suggestions**
- Use `env!("CARGO_BIN_EXE_cargo-pmcp")` to invoke the built CLI.
- Implement a `ChildGuard` with `Drop` kill.
- Print a machine-readable bound-address line from the scaffold, for example `PMCP_SQL_SERVER_ADDR=http://127.0.0.1:...`.
- Fail with captured stdout/stderr on readiness timeout.

**Risk Assessment:** **MEDIUM**. High value test, but subprocess tests are naturally flaky unless cleanup, timeout, and parsing are disciplined.

**86-05**
**Summary:** This is the riskiest plan. The intent is right, but the existing deploy builder does not appear to support a single-crate Lambda project yet, and the Lambda asset/config story is incomplete.

**Strengths**
- Correctly preserves `TargetEntry` and the existing `PmcpRun` variant.
- Reuses existing asset bundling instead of inventing a new deploy path.
- Adds useful detection tests and a D-10 regression guard.
- Keeps real deploy validation for Plan 06.

**Concerns**
- **HIGH:** Current `find_lambda_package_dir` only finds `<server>-lambda` or workspace `*-lambda` packages. A single-crate root project will fail unless `cargo-pmcp/src/deployment/builder.rs` is changed. This file is not in `files_modified`.
- **HIGH:** Bundling `config.toml` and `schema.sql` does not make the generated `main.rs` load them in Lambda. `config.toml` is added at zip root, while assets go under `assets/`; local reads like `"schema.sql"` will not find `assets/schema.sql`.
- **HIGH:** “Deploy uses env token secret” is not actually achieved by putting something in `deploy.toml` unless the runtime config is rewritten or the scaffolded `config.toml` already references env/secrets. As written, bundling `config.toml` may ship the inline dev secret.
- **HIGH:** SQLite DB path is unresolved. D-04 rejects a binary `.db`, but Lambda cannot bootstrap a writable DB under `/var/task/assets`. The deployed DB path likely needs `/tmp/demo.db`, while schema comes from assets.
- **MEDIUM:** `is_config_driven_project` in `deploy/mod.rs` is bin-only. An integration test cannot call a private or `pub(crate)` function unless it is moved to lib/test-support or tested as a unit test in the same module.
- **MEDIUM:** `get_target_id` does not detect pmcp.run from project shape. The scaffolded `deploy.toml` must set `target_type = "pmcp-run"` or tests must pass `--target-type pmcp-run`.

**Suggestions**
- Add `cargo-pmcp/src/deployment/builder.rs` to this plan and explicitly support “project root is the Lambda package.”
- Make the generated runtime use an asset-aware resolver:
  - local config/schema from project root
  - Lambda config from `/var/task/config.toml` or `pmcp::assets`
  - schema from `/var/task/assets/schema.sql`
  - SQLite DB from `/tmp/demo.db`
- Decide how deploy replaces the inline dev secret. Best options: emit a separate deploy config, support env expansion with a default only for local, or rewrite bundled config during deploy.
- Put detection tests inside `deploy/mod.rs` unit tests, or expose a small test-support module from `cargo-pmcp/src/lib.rs`.
- Add a packaging test that inspects the generated zip paths, not just the TOML.

**Risk Assessment:** **HIGH**. Shape D can fail even if all tests except the gated real deploy pass, because build resolution, asset paths, DB writability, and secret posture are not fully specified.

**86-06**
**Summary:** The env-gated real deploy test matches the user decision, but it depends on unresolved Shape D mechanics and misses tempdir dependency patching.

**Strengths**
- Correctly skips by default with `PMCP_RUN_DEPLOY_TEST`.
- Treats the real pmcp.run deploy as the SC-4 deliverable, consistent with D-11.
- Avoids adding an always-on mock contrary to the decision.
- Calls out external requirements like creds and `cargo lambda`.

**Concerns**
- **HIGH:** The scaffolded tempdir project will still need `[patch.crates-io]` overrides for unpublished crates before `cargo lambda build`.
- **HIGH:** Calling deploy in-process is dangerous if the deploy path uses `std::process::exit` on post-deploy failure. The test should shell out to the CLI.
- **MEDIUM:** URL capture is underspecified. If relying on deploy exit code, the test proves lifecycle only indirectly. If calling `run_post_deploy_tests` itself, it needs a reliable deployed URL.
- **MEDIUM:** No teardown strategy is defined. A disposable name helps, but repeated gated runs can still accumulate live resources.
- **LOW:** Consider adding `#[ignore]` in addition to the env gate if this repo convention uses ignored cloud tests.

**Suggestions**
- Reuse the TEST-05 patching helper.
- Invoke the built `cargo-pmcp` binary as a subprocess and assert exit code.
- Use JSON output or a known deployment info file for URL capture.
- Add optional cleanup through `cargo pmcp deploy destroy --yes` when the deploy succeeds, while tolerating cleanup failure in a clearly logged way.

**Risk Assessment:** **MEDIUM-HIGH**. The skip path is safe, but the gated path is likely to fail until Plan 05’s build/assets/secrets story is made concrete.

**Phase-Level Suggestions**
- Add a shared `sql_server_template` source of truth for `main.rs`, config, and schema so the example and scaffold cannot drift.
- Make schema bootstrap idempotent across Shape B, Shape C, and TEST-05.
- Resolve Lambda paths before implementing deploy tests: config path, schema path, SQLite writable DB path, and secret source.
- Add `builder.rs` single-crate Lambda support explicitly, with tests.
- Keep the real deploy test gated, but add non-cloud packaging tests that inspect the artifact zip and build target selection.

---

## Consensus Summary

Both reviewers agree the phase is **well-decomposed and strategically sequenced** — proving the Shape C wiring (86-02) before the Shape B scaffold (86-03) emits a byte-identical copy is called out by both as the standout design choice, and the automated ≤15-line assertion + `[patch.crates-io]` handling of the unpublished toolkit are praised. The divergence is on **overall risk**: Gemini rates the phase LOW (one in-flight type fix), while Codex rates it MEDIUM–HIGH, concentrated almost entirely on Shape D / deploy (86-05, 86-06).

### Agreed Strengths
- **Blueprint-first sequencing** — build the Shape C example, then emit it from the scaffold (both reviewers).
- **Automated ergonomics enforcement** — the line-count assertion validates the ≤15-line non-functional goal with the same rigor as behavior (both).
- **Surgical deploy integration** — detection-based Shape D preserves the `TargetEntry` enum unchanged per D-10 (both).
- **Authentic, env-gated real deploy proof** consistent with D-11; no always-on mock (both).
- Toolkit builder chain used directly instead of re-implementing `pmcp-sql-server::build_server`, and `pmcp_sql_server::serve` correctly avoided (both).

### Agreed Concerns (highest priority — feed into `--reviews` replan)
1. **[HIGH] Lambda asset / file-path / DB-writability story is incomplete and shared across plans.** The same generated `main.rs` is the local `cargo run` entry AND the Lambda entry. Reading `schema.sql`/`config.toml` from a crate-root relative path will not resolve on Lambda (assets land under `/var/task/assets/`, config at zip root), and a SQLite DB cannot be bootstrapped under the read-only `/var/task` — it needs `/tmp`. Gemini flags the path rigidity (MEDIUM, suggests `${DB_PATH:demo.db}` env expansion); Codex flags it as multiple HIGHs across 86-02/03/05. **Decide the asset-aware path resolver (local vs `pmcp::assets`/`/var/task`/`/tmp`) before implementing 86-02, since 86-03 and 86-05 inherit it.**
2. **[HIGH] `execute_batch` type-surface mismatch (Gemini) + schema idempotency (Codex).** Gemini: `execute_batch` is an inherent method on `SqliteConnector` but Plan 02 / Research Example call it on an `Arc<dyn SqlConnector>` — won't compile; call it on the concrete connector before wrapping in `Arc<dyn>`. Codex: the bundled `schema.sql` must be idempotent (`CREATE TABLE IF NOT EXISTS` / `INSERT OR IGNORE`) or the example/scaffold fails on second run against a persisted `demo.db`.
3. **[HIGH] 86-05 likely needs `cargo-pmcp/src/deployment/builder.rs` in scope (Codex).** `find_lambda_package_dir` currently only resolves `<server>-lambda` / workspace `*-lambda` packages — a single-crate root project will not be found. This file is not in 86-05 `files_modified`. The 86-01 Task 2 spike must answer this concretely; if "no", builder.rs must be added.
4. **[HIGH] Deploy secret posture (Codex).** Bundling the scaffold's `config.toml` as-is ships the inline DEV `token_secret`. Deploy must replace it (separate deploy config, env expansion with local-only default, or rewrite-on-deploy). Putting something in `deploy.toml` does not by itself achieve "deploy uses a secrets ref" (D-06).
5. **[MEDIUM] Subprocess test discipline (both, esp. Codex on 86-04/86-06).** Add a `ChildGuard`/Drop-kill to avoid leaking server processes; invoke the real built binary via `env!("CARGO_BIN_EXE_cargo-pmcp")` rather than weakening to in-process `new::execute` (TEST-05 should exercise the actual command surface); emit a machine-readable bound-address line; fail with captured stdout/stderr on readiness timeout; reuse the TEST-05 patch helper in 86-06.
6. **[MEDIUM] Cargo manifest ordering hazard (Codex, 86-01).** Registering `[[example]] name = "sql_server_http"` before the example file exists can make Cargo reject the manifest. Move the `[[example]]` registration to 86-02, or create a placeholder file in 86-01.
7. **[MEDIUM] `is_config_driven_project` / `get_target_id` testability + pmcp.run detection (Codex, 86-05).** A bin-only `pub(crate)` detection fn can't be called from an integration test (use an in-module unit test or a test-support export); `get_target_id` doesn't infer pmcp.run from project shape, so the scaffolded `deploy.toml` must set `target_type = "pmcp-run"` (or tests pass `--target-type`).

### Phase-Level Convergence (both reviewers independently suggest)
- **Single source-of-truth template** for `main.rs` + `config.toml` + `schema.sql` so the Shape C example and the Shape B scaffold cannot drift (Codex explicit; Gemini "unified serve helper" in the same spirit). Consider a thin `pmcp_server_toolkit::serve` helper to shave the line budget and centralize the boilerplate.
- **Resolve Lambda paths first** (config path, schema path, writable SQLite DB path, secret source), then implement the deploy tests.
- **Add non-cloud packaging tests** that inspect the generated artifact zip paths and build-target selection, in addition to the gated real-deploy test.

### Divergent Views
- **Overall risk level:** Gemini = LOW ("ready for execution provided the Plan 02 type mismatch is corrected in-flight"); Codex = MEDIUM–HIGH (Shape D build/assets/secrets/DB story is under-specified and several non-gated 86-05/06 paths could fail even when the gated real deploy is skipped). The honest read: Shapes B/C are low-risk and ready; **Shape D (86-05/86-06) is the concentration of real risk** and warrants a targeted replan or an expanded 86-01 spike before execution.
- **Line-count definition:** Codex wants the "≤15 lines" rule defined precisely (physical non-empty body lines; imports/attributes excluded) and shared with the verifier; Gemini accepts the body-count assertion as-is.
