# Phase 86: Shapes B/C/D — Scaffold, Library Example, Deploy - Context

**Gathered:** 2026-05-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Wrap three developer-ergonomics "shapes" around the proven Phase 85
`pmcp-sql-server` pure-config core, for non-pure-config use cases:

- **Shape B (SHAP-B-01 / TEST-05):** `cargo pmcp new --kind sql-server`
  scaffolds a single runnable starter project (`Cargo.toml` pinning the
  toolkit + SQLite backend, `main.rs` with the ≤15-line Shape-C wiring,
  commented `config.toml` template, bundled `schema.sql`). `cargo run`
  against an embedded SQLite serves `tools/list` + at least one `tools/call`
  out of the box, verified end-to-end by a tempdir integration test.
- **Shape C (SHAP-C-01):** a runnable example proving library use of
  `pmcp-server-toolkit` (+ SQLite connector) — a complete MCP server in
  **≤15 lines** of `main.rs`, the same wiring the Shape B scaffold emits.
- **Shape D (SHAP-D-01 / TEST-06):** `cargo pmcp deploy` packages a
  config-only server as a pure-Rust Lambda binary and deploys it to pmcp.run,
  with the Phase 77 `cargo pmcp configure` target system accommodating it
  **without breaking changes** to existing target variants.

**In scope:**

- Teach `cargo pmcp new` a `--kind sql-server` path that emits a single
  runnable crate (distinct from `new`'s current multi-crate workspace
  scaffolding). SQLite-only backend for now.
- A bundled `schema.sql` (DDL + a few INSERTs) that bootstraps the demo DB on
  first run; the DDL doubles as the `--schema`/code-mode schema resource.
- The ≤15-line wiring shape: explicit `pmcp-server-toolkit` + SQLite-connector
  wiring (load config+schema → connector → `ServerBuilderExt` /
  `code_mode_from_config` → serve **HTTP**), shared by the Shape C example and
  the Shape B scaffold `main.rs`.
- A runnable SQLite Shape C example (CI-runnable, zero creds).
- `cargo pmcp deploy` per-project build for config-driven projects: detect a
  config-driven project (config.toml + schema present), build its own crate to
  a pure-Rust Lambda binary, bundle `config.toml` + `schema.sql` as deploy
  assets (`pmcp::assets` path resolution); reuse the existing `PmcpRun` target
  unchanged (detection-based, **zero enum changes**).
- TEST-06 deploy integration test against a **real pmcp.run target behind a
  creds/env gate** (authentic; skipped in normal CI) running the Phase 79
  post-deploy lifecycle (`check` + `conformance` + `apps` verifier).
- Generated `config.toml` ships with `[code_mode] enabled = true` (v2.2's
  headline NL→SQL surface) + an inline DEV-ONLY `token_secret` so the demo
  works on first `cargo run`.
- ALWAYS coverage (CLAUDE.md): unit + property + integration + doctests +
  the runnable Shape C example + fuzz reuse.

**Out of scope (other phases own these):**

- `pmcp-config-helper` Type 2 authoring Skills MCP server → Phase 87.
- `crates/pmcp-server` dogfood rewrite → Phase 88.
- Book chapter / course tutorial / migration recipe / README config-first
  positioning → Phase 89 (the cargo-pmcp CLI README todo is folded for *new
  command surfaces touched here*, but the broad README rewrite stays Phase 89).
- Non-SQLite scaffold backends (`--backend postgres|mysql|athena`) — deferred;
  `--kind sql-server` is SQLite-only this phase.
- Removing/replacing the existing 526-line Rust `sqlite-explorer` template —
  it stays as the Rust escape-hatch, untouched.
- stdio transport — Phase 86 commits to HTTP everywhere (scaffold, example,
  Lambda). Phase 85's deferred stdio stays deferred.
- Universal-binary / platform-hosted config-only deploy model — rejected in
  favor of per-project build (see D-13).

</domain>

<decisions>
## Implementation Decisions

### Shape B — Command Surface (SHAP-B-01, TEST-05)

- **D-01:** **`cargo pmcp new --kind sql-server` is the command, taken
  literally.** Both SC-1 and the TEST-05 description name this exact
  invocation, so honor it verbatim rather than the captured todo's
  `add --template sqlite-explorer-config` alternative. Teach `new` a `--kind`
  path that produces a **single runnable crate** (`Cargo.toml` + `main.rs` +
  `config.toml` + `schema.sql`), distinct from `new`'s current behavior of
  scaffolding a multi-crate workspace (`crates/`, `scenarios/`, `lambda/`).
- **D-02:** **SQLite-only backend for `--kind sql-server` this phase.** SQLite
  is the only zero-creds backend (makes `cargo run` + TEST-05 work out of the
  box). `--backend postgres|mysql|athena` is a future additive sub-flag, not
  Phase 86.
- **D-03:** **Keep the existing 526-line Rust `sqlite-explorer` template
  (`add --template sqlite-explorer`) untouched** as the Rust-driven /
  customizable escape-hatch. The new config-driven `--kind sql-server` is the
  v2.2 default. Two audiences, per the folded todo — do NOT remove or deprecate
  the Rust template in this phase.

### Shape B — Scaffold Contents & Embedded DB (SHAP-B-01, TEST-05)

- **D-04:** **Bundled `schema.sql` (DDL + a few INSERTs) bootstraps the demo
  SQLite DB on first run.** Git-friendly, deterministic, zero external files;
  the same DDL doubles as the `--schema`/code-mode schema resource. Rejected:
  a binary `.db` blob in the template tree (heavier), and a `${SQLITE_DB_PATH}`
  env-only path (weakens SC-1's out-of-the-box promise).
- **D-05:** **HTTP transport everywhere** — the scaffold's `main.rs` serves
  streamable HTTP (matching the Phase 85 binary and the Shape D Lambda target).
  One transport for scaffold + example + deploy. TEST-05 therefore spawns the
  scaffolded server on a local address and polls for readiness, then exercises
  `tools/list` + one `tools/call`. **No stdio support added** (Phase 85's
  deferred stdio stays deferred).

### Shape B/C — Generated config.toml posture (SHAP-B-01)

- **D-06:** **`[code_mode] enabled = true` by default in the generated
  config**, with an **inline DEV-ONLY `token_secret`** and a loud
  "DEV ONLY — replace via a secrets ref for production" comment. This
  showcases v2.2's headline NL→SQL (`validate_code`/`execute_code`) surface on
  first `cargo run` without requiring the user to set an env var. The Shape D
  deploy path sources `token_secret` from a secrets ref, not the inline dev
  default. Rejected: `enabled = false` opt-in block (hides the headline
  feature), and `${CODE_MODE_SECRET}` env-only (breaks out-of-box run).

### Shape C — ≤15-line API Shape (SHAP-C-01)

- **D-07:** **Explicit `pmcp-server-toolkit` + connector wiring in ≤15 lines**
  (load config+schema → build SQLite connector → `ServerBuilderExt` /
  `code_mode_from_config` → serve HTTP). This is genuine "library use of the
  toolkit" per SHAP-C-01's literal wording, and is the **same shape the Shape B
  scaffold `main.rs` emits** (SC-1 says scaffold main.rs *is* the Shape C
  wiring). Relies on Phase 85's builder ergonomics (`ServerBuilderExt`) being
  tight enough to fit ≤15 lines. Rejected: a `pmcp_sql_server::run(config)`
  one-liner (that demonstrates *using pmcp-sql-server*, not *library use of the
  toolkit + connector*).
- **D-08:** **SQLite-only runnable example.** The example uses the toolkit's
  built-in SQLite connector and runs fully in CI (zero creds).
  **Deviation note for the verifier:** SHAP-C-01's "+ a chosen
  `pmcp-toolkit-<backend>` crate" clause is satisfied *in intent* by the SQLite
  feature (SQLite's connector lives inside `pmcp-server-toolkit`, not a separate
  `pmcp-toolkit-*` crate). The separate connector crates (postgres/mysql/athena)
  need creds and cannot `cargo run` in CI. Record this as an approved scope
  reading, not a gap. (Mirrors Phase 85's REF-02 open-images deviation
  precedent.)

### Shape D — Deploy Packaging & Target Wiring (SHAP-D-01, TEST-06)

- **D-09:** **Per-project build + asset bundle.** `cargo pmcp deploy` detects a
  config-driven project (presence of `config.toml` + schema) and builds *its
  own* ≤15-line crate into a pure-Rust Lambda binary, bundling `config.toml` +
  `schema.sql` as deploy assets resolved via `pmcp::assets`. Reuses the
  existing deploy pipeline. Rejected: shipping the prebuilt universal
  `pmcp-sql-server` binary + uploading only config/schema (truest "config-only"
  but needs pmcp.run platform-side hosting changes).
- **D-10:** **Existing `PmcpRun` target used unchanged — zero enum changes.**
  The Phase 77 `TargetEntry` tagged enum (`PmcpRun`/`AwsLambda`/
  `GoogleCloudRun`/`CloudflareWorkers`) is NOT modified. Config-only-server
  support is **detection-based** in the deploy command, which satisfies
  SHAP-D-01's "without breaking changes" constraint by making no schema change
  at all. No new variant, no new field.
- **D-11:** **TEST-06 runs against a REAL pmcp.run target behind a creds/env
  gate** — authentic deploy + Phase 79 post-deploy lifecycle (`check` +
  `conformance` + `apps` verifier), skipped in normal CI when the gate is
  absent. SC-4 explicitly permits "a mock or real pmcp.run target", so the
  real-gated path satisfies it. **Tension to record (not a gap):** there is no
  deploy assertion running on *every* PR — CI stays green without creds, and
  the authentic check is opt-in. The verifier should treat the env-gated test
  as the SC-4 deliverable, not flag the absence of an always-on mock.

### Claude's Discretion

- The scaffold's exact `config.toml` comment text and the demo table
  schema/seed rows in `schema.sql` (e.g., a small books/movies/notes table).
- The `--kind` plumbing in `new.rs` (new code path vs branch in `execute`),
  template module layout under `cargo-pmcp/src/templates/`, and how the
  single-crate output differs structurally from the workspace path.
- Exact default HTTP bind address/port for the scaffolded server and how
  TEST-05 detects readiness (prefer a poll, per Phase 85 D-15 precedent).
- The inline DEV-ONLY `token_secret` literal value and the precise wording of
  the "replace for production" comment.
- How `cargo pmcp deploy` detects "config-driven project" (heuristic: presence
  of `config.toml` + `schema.sql` + the `pmcp-server-toolkit` dep).
- The env-var/feature name that gates TEST-06 (e.g., `PMCP_RUN_DEPLOY_TEST`).
- The exact ≤15-line example file name/location (must dep on toolkit + sqlite;
  likely `crates/pmcp-server-toolkit/examples/` — root `pmcp` `examples/` may
  lack the toolkit/sqlite deps; planner confirms).

### Folded Todos

- **`2026-05-18-ship-sqlite-from-config-example-as-phase-86-shape-b-c-dogfood.md`**
  — proposed a config-driven SQLite template (~50-line TOML) to replace the
  526-line hand-coded Rust template for the config-only case, plus a Shape C
  `examples/sqlite_from_config.rs`. **Folded:** directly informs Shapes B & C.
  Note the command surface decision (D-01) chose `new --kind sql-server` over
  the todo's `add --template sqlite-explorer-config`; D-03 keeps the Rust
  template as escape-hatch per the todo's "two templates, not one" guidance.
  The todo also surfaces the SQLite `:table` identifier-substitution concern —
  out of Phase 86's scope (SqlConnector trait work; verify Phase 84 already
  handles it for the curated `[[tools]]` used in the scaffold).
- **`2026-03-04-create-readme-docs-for-cargo-pmcp-cli.md`** — "Create README
  docs for cargo-pmcp CLI." **Folded (scoped):** document the *new command
  surfaces this phase touches* (`new --kind sql-server`, config-only
  `deploy`) in the cargo-pmcp README/help. The broad/full cargo-pmcp README
  rewrite remains Phase 89 (DOCS / config-first positioning) — only the Phase
  86 additions are in scope here.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Dependency — Read First

- `.planning/phases/85-shape-a-pure-config-binary-reference-parity/85-CONTEXT.md`
  — the Phase 85 decisions Phase 86 builds on: `pmcp-sql-server` binary shape,
  SQLite as the zero-creds backend, HTTP-only transport (and its deferred
  stdio), all-four-connectors-default-on, `ServerConfig` + `ServerBuilderExt`
  wiring, `--schema` semantics.
- `crates/pmcp-sql-server/` — the Phase 85 crate Shape B/C/D wrap. Read its
  `main.rs` to extract the ≤15-line wiring shape (D-07); note it was built
  binary-only (no lib `run()` per D-07's rejection).

### Toolkit Core + Connector (the ≤15-line wiring surface)

- `crates/pmcp-server-toolkit/src/builder_ext.rs` — `ServerBuilderExt` /
  `code_mode_from_config`; the ergonomics that must fit ≤15 lines (D-07).
- `crates/pmcp-server-toolkit/src/config.rs` — `ServerConfig` parser
  (`deny_unknown_fields`, env expansion) the scaffold's `config.toml` targets.
- `crates/pmcp-server-toolkit/src/sql/mod.rs` — `SqlConnector` trait, `Dialect`,
  `SqliteConnector`, `translate_placeholders` (the in-toolkit SQLite connector
  used by D-08).
- `crates/pmcp-server-toolkit/src/code_mode.rs` — `build_code_mode_prompt` /
  `assemble_code_mode_prompt`; where the `schema.sql` DDL feeds in (D-04, D-06).
- `crates/pmcp-server-toolkit/src/tools.rs` — `synthesize_from_config` for the
  curated `[[tools]]` the scaffold ships.

### Shape B — cargo-pmcp scaffold integration points

- `cargo-pmcp/src/commands/new.rs` — the `execute` entry to extend with
  `--kind sql-server` (D-01); currently scaffolds a multi-crate workspace.
- `cargo-pmcp/src/commands/add.rs` — existing `--template` dispatch
  (`print_template_details`, `print_sqlite_explorer_template_details`,
  `print_try_it_out`); the existing `sqlite-explorer` escape-hatch lives here.
- `cargo-pmcp/src/templates/sqlite_explorer.rs` — the 526-line Rust template
  kept untouched (D-03); reference for what the config-driven scaffold replaces.
- `cargo-pmcp/src/templates/` (`workspace.rs`, `server_common.rs`, `mod.rs`) —
  template-generation patterns to mirror for the new single-crate kind.

### Shape D — deploy + Phase 77 configure target system

- `cargo-pmcp/src/commands/configure/config.rs` — `TargetEntry` tagged enum +
  per-variant entries (`PmcpRunEntry` etc.); D-10 leaves this UNCHANGED.
- `cargo-pmcp/src/commands/deploy/` (`deploy.rs`, `mod.rs`, `init.rs`) — the
  deploy pipeline to extend with config-driven-project detection + asset
  bundling (D-09).
- `cargo-pmcp/src/deployment/post_deploy_tests.rs` — Phase 79 post-deploy
  lifecycle (`check` + `conformance` + `apps`) that TEST-06 must run cleanly.
- `cargo-pmcp/src/deployment/` (`trait.rs`, `registry.rs`, `operations.rs`,
  `outputs.rs`) — deploy trait + registry the per-project build plugs into.

### Reference Config (config.toml shape for the scaffold template)

- `pmcp-run/built-in/sql-api/servers/open-images/config.toml` — the verified
  config shape (`[server]`, `[metadata]`, `[database]`, `[[database.tables]]`,
  `[code_mode]`, `[[tools]]` with named `:param` bindings, `[[prompts]]`); the
  scaffold's SQLite `config.toml` is a small analog of this.
- `pmcp-run/built-in/sql-api/reference/config.toml` — the SQLite Chinook
  reference config (closest existing SQLite analog).

### Requirements & Roadmap

- `.planning/REQUIREMENTS.md` — SHAP-B-01, SHAP-C-01, SHAP-D-01, TEST-05,
  TEST-06 (lines 158–160, 205–206).
- `.planning/ROADMAP.md` §"Phase 86" (lines 1504–1516) — goal, depends-on
  (Phase 85), SC-1..4.

### Memory & Conventions

- `.claude/projects/-Users-guy-Development-mcp-sdk-rust-mcp-sdk/memory/feedback_avoid_docker_pure_rust_lambda.md`
  — pure-Rust Lambda, no Docker/testcontainers; authentic in-process mocks.
- `CLAUDE.md` §"Release & Publish Workflow" — crate publish order; §"ALWAYS
  Requirements" + §"PMAT Quality-Gate" — coverage matrix + cognitive
  complexity ≤25.
- `.claude/skills/spike-findings-rust-mcp-sdk/` — schema-server architecture +
  dual-surface invariant (auto-loaded).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **Phase 85 `pmcp-sql-server` `main.rs`** — the existing config→connector→
  serve flow to distill into the ≤15-line Shape C/B wiring (D-07).
- **`ServerBuilderExt` / `code_mode_from_config`** (toolkit, Phase 85) — the
  one-call wiring that makes ≤15 lines feasible.
- **Toolkit built-in `SqliteConnector`** (`src/sql/mod.rs`) — pure-Rust
  `rusqlite bundled`; the runnable example/scaffold backend (D-02, D-08).
- **Existing cargo-pmcp template machinery** (`templates/workspace.rs`,
  `server_common.rs`, `add.rs` printers) — mirror for the new `--kind` output.
- **Phase 77 `TargetEntry` + configure resolver** — the deploy target system
  Shape D plugs into *without* modifying (D-10).
- **Phase 79 post-deploy verifiers** (`post_deploy_tests.rs`) — TEST-06 reuses
  these against the real-gated pmcp.run target.

### Established Patterns

- **`#[serde(deny_unknown_fields)]` strict config parsing** — the scaffold's
  generated `config.toml` must parse cleanly through the toolkit `ServerConfig`.
- **Env-var expansion in config** (`${...}`) — used for the Shape D
  `token_secret` secrets-ref path (D-06).
- **Per-backend cargo feature gating** — the scaffold's `Cargo.toml` pins the
  toolkit with the `sqlite` feature.
- **`--test-threads=1`** (project CI convention) — TEST-05 spawns an HTTP
  server + client; keep single-threaded-safe.
- **Env-gated integration tests** — TEST-06 follows the pattern of skipping
  when its creds/env gate is absent (D-11).

### Integration Points

- **`cargo pmcp new` `execute` ← `--kind sql-server`** — the new dispatch (D-01).
- **scaffold `main.rs` == Shape C example** — one wiring shape, two emitters
  (D-07).
- **`cargo pmcp deploy` ← config-driven-project detection** — the new seam that
  triggers asset bundling without touching the target enum (D-09/D-10).
- **TEST-06 ← real pmcp.run (gated) ← Phase 79 verifiers** — the authentic
  deploy integration point (D-11).
- **bundled `schema.sql` → demo DB bootstrap + code-mode schema resource** —
  one file, two roles (D-04).

</code_context>

<specifics>
## Specific Ideas

- **Honor the contract verbatim (user choice):** `cargo pmcp new --kind
  sql-server` because both SC-1 and the TEST-05 description name that exact
  command — even though `new` today builds a workspace, not a single crate.
- **Two templates, not one (folded todo + user choice):** the config-driven
  `--kind sql-server` is the v2.2 default; the 526-line Rust `sqlite-explorer`
  stays as the customizable escape-hatch.
- **Headline feature visible on first run (user choice):** generated config
  ships `[code_mode] enabled = true` with an inline DEV-ONLY `token_secret` and
  a loud "replace for production" note, so `cargo run` demonstrates NL→SQL
  immediately; deploy uses a secrets ref instead.
- **One transport everywhere (user choice):** HTTP for scaffold + example +
  Lambda, rather than reaching for local stdio.
- **Authentic deploy proof (user choice):** TEST-06 hits a real pmcp.run target
  behind a creds/env gate rather than an always-on mock — authentic over
  always-green; CI without creds simply skips it.

</specifics>

<deferred>
## Deferred Ideas

- **Non-SQLite scaffold backends** (`cargo pmcp new --kind sql-server --backend
  postgres|mysql|athena`) — additive sub-flag; deferred past Phase 86 (those
  backends need creds and can't `cargo run` cleanly). (D-02.)
- **`pmcp_sql_server::run(config, schema)` library convenience** — a one-call
  entry extracted from the Phase 85 binary; rejected for the SHAP-C-01 example
  (D-07) but could land additively as a convenience later.
- **Always-on mock pmcp.run deploy test** — Phase 86 chose the real-gated path
  (D-11); a mock-target CI gate could be added later if an always-on deploy
  assertion is wanted.
- **stdio transport for local scaffold/example** — Phase 86 commits to HTTP
  everywhere (D-05); stdio remains deferred from Phase 85.
- **Deprecating/removing the Rust `sqlite-explorer` template** — kept as
  escape-hatch this phase (D-03); revisit in the Phase 89 docs/cleanup pass.
- **Broad cargo-pmcp CLI README rewrite** — only the Phase 86 command-surface
  additions are documented here; the full README + config-first positioning is
  Phase 89 (DOCS).
- **SQLite `:table` identifier-substitution in `SqlConnector`** (raised by the
  folded dogfood todo) — a connector-trait concern, not Phase 86; confirm
  Phase 84 already covers the curated `[[tools]]` the scaffold ships.

### Reviewed Todos (not folded)

None — both matched todos were folded (one fully, one scoped to this phase's
new command surfaces; see Folded Todos).

</deferred>

---

*Phase: 86-shapes-b-c-d-scaffold-library-example-deploy*
*Context gathered: 2026-05-26*
