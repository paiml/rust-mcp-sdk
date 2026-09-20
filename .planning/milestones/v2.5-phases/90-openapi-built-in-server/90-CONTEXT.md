# Phase 90: OpenAPI Built-In Server (`pmcp-openapi-server`) - Context

**Gathered:** 2026-05-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver a config-driven **OpenAPI/REST** MCP server that mirrors the completed SQL
toolkit (Shape A binary `pmcp-sql-server`, Phases 83–86): a non-developer points a
binary at a `config.toml` (+ optionally an OpenAPI spec) and gets a live MCP server
over streamable HTTP — curated `[[tools]]` for the common ~20%, Code Mode for the
long-tail ~80% — with **zero Rust written**.

The backend-agnostic toolkit (Phase 83) and the Shape A / scaffold / deploy
patterns (Phases 85–86) are **reused**. New surface is limited to: an HTTP/OpenAPI
connector + executor seam, the operation/script → tool config mapping, the
`pmcp-openapi-server` Shape A binary, the `cargo pmcp new --kind openapi-server`
scaffold, and docs in three shapes.

**In scope:**

- Lift the genuinely-new HTTP/OpenAPI pieces from the reference
  (`~/Development/mcp/sdk/pmcp-run/built-in/openapi-api/crates/mcp-openapi-server-core`)
  into `pmcp-server-toolkit`: the HTTP connector (reqwest/rustls), the outgoing-HTTP
  auth provider, the openapiv3 spec parser, and the code-mode `HttpExecutor` seam —
  replacing the reference's `shared/mcp-server-common` + `shared/mcp-lambda-proxy`
  path-deps with the public toolkit (the same REF-style lift the SQL toolkit did).
- Additive `[backend]` / `[backend.auth]` / `[backend.http]` config sections on the
  shared toolkit `ServerConfig` (D-06).
- **Two kinds of curated `[[tools]]`** (D-01): single-call (`path`+`method`) AND
  script tools (`script = """<JS>"""`), both executed by the **same JS engine** as
  Code Mode (D-02).
- A `pmcp-openapi-server` Shape A binary (`--config` + optional `--spec` + `--http`),
  structural copy of `pmcp-sql-server`.
- A `cargo pmcp new --kind openapi-server` scaffold, mirroring `--kind sql-server`.
- REF parity: the **london-tube** instance reproduces the pmcp-run server's tools +
  behavior unchanged, replayed offline via wiremock (D-04).
- Docs in three shapes (crate README + `pmcp-book` chapter + `pmcp-course` chapter).
- ALWAYS coverage (CLAUDE.md): unit + property + integration + doctests + runnable
  example + fuzz reuse (extend the config-parser fuzz target for `[backend]`).

**Out of scope (other phases / future milestones own these):**

- A GraphQL built-in server (the sibling third backend) — separate phase/milestone.
- `lichess` as the parity target — kept as an optional secondary demo only (D-04).
- Live-network parity replay against api.tfl.gov.uk in default CI — env-gated only.
- stdio transport — HTTP-only everywhere, carrying forward Phase 85 D-12 / Phase 86 D-05.
- Non-OpenAPI `--kind` scaffold backends; broad cargo-pmcp README rewrite (Phase 89 DOCS).
- DynamoDB/AVP runtime policy overrides — Shape A enforces `[code_mode]` policy
  statically from config (carry-forward Phase 85 D-13).

</domain>

<decisions>
## Implementation Decisions

### Tool Model — Tools Are Promoted Code-Mode Code (THE headline decision)

- **D-01:** **Across all config-driven built-in servers, a curated `[[tools]]`
  entry is *promoted code-mode code*** — an optimized, verified, simplified version
  of a frequently-used operation, distinct from the LLM-generated long-tail that
  Code Mode handles on the fly. The promotion lineage per backend:
  - **SQL** → a tool is a **SQL query** (`sql = "..."`)
  - **GraphQL** → a tool is a **GraphQL query/mutation** (future sibling)
  - **OpenAPI** → a tool is a **JavaScript script** (subset of JS) describing the
    API call(s)
  For the OpenAPI built-in specifically, a curated tool is **one of two kinds**
  (reference `OPENAPI_SCRIPT_TOOLS.md` detection logic):
  - **single-call tool** — `path` + `method` (+ optional per-tool `base_url`): the
    direct one-operation HTTP mapping (the `sql=` analog for the simple case).
  - **script tool** — `script = """<JS>"""`: embedded JavaScript using the
    `api.get(path)` / `api.post(path, body)` syntax for **multi-call domain
    operations** — chaining one call's output into the next, and iterating over
    arrays with `filter`/`map` across steps.
  Detection rule: a `script` field ⇒ script tool; `path`+`method` ⇒ single-call
  tool. Script-tool parameters are declared via `[[tools.parameters]]` and bound to
  an `args` object inside the script (reference design).

- **D-02:** **ONE JavaScript engine for BOTH Code Mode and curated script tools
  (hard requirement).** The same JS executor that translates a script → API calls
  in Code Mode (`validate_code` / `execute_code`, the long-tail) MUST also execute
  curated script tools. No second engine, no divergent semantics — a script that
  works in Code Mode must behave **identically** when promoted to a curated tool.
  Both paths route through the toolkit's HTTP-execution seam feeding the SDK's
  code-mode JS engine. Engine capabilities both surfaces share: multi-call
  chaining (output → input), array iteration (`filter`/`map`), and bounded loops.
  **Planner MUST reconcile the engine identity:** the research (`90-RESEARCH.md`
  Pattern 4) names the SDK's `JsCodeExecutor` (Boa) in `pmcp-code-mode 0.5.1`,
  while the reference design doc shows `PlanCompiler` / `PlanExecutor` /
  `PlanStep::ApiCall` from the `pmcp-code-mode 0.4.0` era. These must be unified on
  the **current SDK engine** so curated script tools and Code Mode share one
  compile+execute path. This is the single most important architectural constraint
  of the phase — the research under-weighted script tools (it framed curated tools
  as `path`/`method` only), so treat the script-tool path as first-class, not as
  the `[tools.outputs]` transform the research mentioned.

### OpenAPI Spec — Dual Usefulness + Runtime Policy

- **D-03:** **The OpenAPI spec is useful in BOTH aspects of MCP server operation**
  and `--spec` is **optional at runtime** (diverges deliberately from SQL's
  effectively-required `--schema`, Phase 85 D-04). Rationale:
  - Curated tools (single-call AND script) carry everything they need to execute,
    so a **curated-only server starts from `config.toml` alone** — no spec file
    required. A public no-auth/low-auth demo must boot without shipping a spec.
  - When a spec **is** provided it serves **both** surfaces: (a) it powers the Code
    Mode `api_schema` resource the LLM reads to generate long-tail scripts, and
    (b) it is available to validate/author curated tool paths and script `api.*()`
    calls against the real API contract.
  - `--spec` is **required at scaffold/discovery time** (`cargo pmcp new --kind
    openapi-server` generates `[[tools]]` from it) and is the natural input
    whenever Code Mode needs the full schema resource.
  The dual usefulness mirrors SQL's `--schema` dual role (code-mode prompt +
  schema resource); only the runtime-optionality differs because REST curated
  tools are self-contained where SQL's code-mode prompt leaned on the DDL.

### Reference Parity Target

- **D-04:** **The canonical REF parity + demo target is `london-tube` (TfL),
  only.** It exercises the `api_key` **query-parameter** outgoing-auth path — the
  genuinely-new auth shape this phase introduces — making it the most valuable
  single parity target. Parity is asserted offline via **wiremock** (pure-Rust, no
  Docker, no live network in default CI); a live-network replay against
  api.tfl.gov.uk is **env-gated** (mirror the SQL Athena `#[ignore]`+env pattern,
  Phase 84). `lichess` (bearer/optional auth) is **not** the parity target — kept
  as an optional secondary demo instance only. mcp-tester replay against the
  reference scenarios is the parity assertion (carry-forward Phase 85 D-10).

### Outgoing-HTTP Auth Variants

- **D-05:** **Ship all five `[backend.auth]` variants in Phase 90** — `none` /
  `api_key` / `bearer` / `basic` / `oauth2_client_credentials` / `oauth_passthrough`
  — lifted **wholesale** from the reference (`config.rs:164-235`), which already
  implements and tests them as a config enum. This is a near-verbatim lift, not new
  code, so "all now" is *less* work than trimming-and-regrowing and is future-proof.
  These are the **outgoing/backend** auth providers (`apply(headers, query)`) —
  distinct from the toolkit's **inbound** MCP-client `AuthProvider::validate_request`
  (research Pitfall 1); name/namespace them to avoid conflation. `oauth_passthrough`
  bridges the two via the pmcp `AuthContext` token-capture pattern.

### Config Shape

- **D-06:** **Additive `[backend]` on the shared toolkit `ServerConfig`.** Extend
  the Phase 83 backend-agnostic `ServerConfig` with `[backend]` / `[backend.auth]`
  / `[backend.http]` additively, preserving `#[serde(deny_unknown_fields)]`. One
  config type spans SQL + OpenAPI — this fulfills the explicit "backend-agnostic
  toolkit" goal of Phase 83. Existing SQL configs are unaffected (additive keys).
  A minor `pmcp-server-toolkit` version bump accompanies the new `http` surface
  (mirror Phase 84's connector-driven bump).

### Carried Forward from Phases 85–86 (locked; not re-discussed)

- **CF-1 (P85 D-12 / P86 D-05):** **Streamable HTTP only** — scaffold, example,
  binary, and Lambda all serve streamable HTTP (Phase 56 Tower/axum adapter).
  stdio stays deferred.
- **CF-2 (P85 D-09):** **Lazy startup** — the HTTP connector is constructed without
  contacting the backend or requiring live creds; real I/O surfaces on first call.
- **CF-3 (P86 D-01/D-03):** **`cargo pmcp new --kind openapi-server` honored
  verbatim**, emitting a single runnable crate; existing templates stay as the Rust
  escape-hatch, untouched.
- **CF-4 (P86 D-06):** Generated `config.toml` ships **`[code_mode] enabled = true`**
  with an inline **DEV-ONLY `token_secret`** + a loud "replace for production" note,
  so `cargo run` demonstrates the long-tail JS surface immediately; deploy sources
  the secret from a secrets ref.
- **CF-5 (P86 D-07):** The **≤15-line wiring shape** (load config[+spec] → connector
  → `ServerBuilderExt` → serve HTTP) is shared by the Shape C example and the Shape
  B scaffold `main.rs`.
- **CF-6 (P86 D-09/D-10):** Deploy via **per-project build + asset bundle**; the
  Phase 77 `PmcpRun`/target enum stays **unchanged** (detection-based). Spec/config
  are read-only deploy assets via `pmcp::assets`.
- **CF-7 (P85 D-13):** **Static `[code_mode]` policy from config** — no
  DynamoDB/AVP runtime overrides.
- **CF-8 (P85 D-03/D-04/D-05):** **Two-input model** — `config.toml` (server +
  operations + `[code_mode]` policy) and an admin-authored, editable/redactable
  schema file (the OpenAPI spec) surfaced as the code-mode resource.

### Claude's Discretion

- `HttpConnector` as a `trait` (`Arc<dyn HttpConnector>`) vs a concrete struct —
  research recommends a trait for parity with `SqlConnector` and feature-gating;
  lean trait unless the planner finds it pure over-engineering.
- Exact `script` config field name and the `[[tools.parameters]]` → `args` binding
  shape (mirror the reference `OPENAPI_SCRIPT_TOOLS.md` design).
- Script-tool `ExecutionConfig` bounds (`max_api_calls`, `max_loop_iterations`,
  timeout) and their defaults.
- Feature-gating Code Mode / `js-runtime` on the binary (default-on, opt-out via
  `--no-default-features`) — mirror `pmcp-sql-server`'s `code-mode` feature to keep
  curated-only builds light (research Pitfall 4).
- URL building: lift the reference's explicit path-concat (NOT `Url::join`, which
  drops API-Gateway stage prefixes — research Pitfall 2).
- Error-redaction wording in `HttpConnectorError` / `DispatchError` (must never
  echo URL or `Authorization` — research Pitfall 5); the wiremock fixture shape for
  the london-tube replay; default HTTP bind address/port and readiness-poll method.

### Folded Todos

- **`2026-03-04-create-readme-docs-for-cargo-pmcp-cli.md`** — "Create README docs
  for cargo-pmcp CLI." **Folded (scoped):** document the **new `--kind
  openapi-server` command surface** in the cargo-pmcp README/help (mirrors the
  identical scoped fold in Phase 86 for `--kind sql-server`). The broad cargo-pmcp
  README rewrite / config-first positioning remains Phase 89 (DOCS) — only the
  Phase 90 command-surface addition is in scope here.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Research & Dependencies — Read First

- `.planning/phases/90-openapi-built-in-server/90-RESEARCH.md` — the full lift map:
  Standard Stack, "What's New vs What's Reused" table, the `HttpConnector` /
  `HttpCodeExecutor` trait analogs, 6 pitfalls, code examples. **Caveat:** the
  research framed curated `[[tools]]` as `path`/`method` operation mappings and
  under-weighted **script tools** (D-01/D-02) — treat script tools as first-class.
- `.planning/phases/85-shape-a-pure-config-binary-reference-parity/85-CONTEXT.md` —
  the Shape A binary shape, two-input model, lazy startup, HTTP-only,
  parity-replay-as-assertion, static code-mode policy (CF-1/2/7/8 origins).
- `.planning/phases/86-shapes-b-c-d-scaffold-library-example-deploy/86-CONTEXT.md` —
  `--kind` scaffold, ≤15-line wiring, code_mode-enabled generated config,
  per-project deploy with unchanged target enum (CF-3/4/5/6 origins).
- `.planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/83-CONTEXT.md` — the
  backend-agnostic `ServerConfig`, code-mode wiring, REF-01 superset enforcement
  (the surface `[backend]` extends, D-06).

### Reference to Lift From (source of truth — pmcp-run openapi-api)

- `~/Development/mcp/sdk/pmcp-run/built-in/openapi-api/OPENAPI_SCRIPT_TOOLS.md` —
  **the tools-are-scripts model (D-01/D-02)**: single-call vs script-tool detection,
  `script`/`[[tools.parameters]]`/`args` config, `ScriptToolHandler` reusing the
  same `PlanCompiler`/`PlanExecutor`+`HttpExecutor` as Code Mode. The design anchor
  for the headline decision.
- `~/Development/mcp/sdk/pmcp-run/built-in/openapi-api/crates/mcp-openapi-server-core/src/http/mod.rs`
  — `HttpClient::execute_with_options` (the connector body to lift; path-concat at
  ~`:212-218`, header injection guards at ~`:138`).
- `.../mcp-openapi-server-core/src/auth/mod.rs` + `src/config.rs` (`:164-235`) — the
  five outgoing-auth variants (D-05) and the `[backend.auth]` config enum.
- `.../mcp-openapi-server-core/src/code_mode.rs` — `HttpClientExecutor` impl of the
  code-mode executor trait (the seam for D-02); the body to lift for `HttpExecutor`.
- `.../mcp-openapi-server-core/src/tools/mod.rs` — `create_tool_from_config`
  (single-call) + output transforms; where script-tool registration is added.
- `.../mcp-openapi-server-core/src/schema/parser.rs` — openapiv3 spec parsing (D-03).
- `.../mcp-openapi-server-core/src/pmcp_server.rs` — **do NOT lift verbatim**
  (research Pitfall 6); replace with the toolkit `assemble.rs` pattern.
- `~/Development/mcp/sdk/pmcp-run/built-in/openapi-api/instances/london-tube.toml`
  and `.../servers/london-tube/` — the parity fixture + reference output (D-04;
  api_key query-param auth).
- `~/Development/mcp/sdk/pmcp-run/built-in/openapi-api/OPENAPI_CODE_MODE_DESIGN.md`,
  `OPENAPI_CODE_MODE_POLICY_DESIGN.md`, `OPENAPI_CODE_MODE_ACCESS_CONTROL.md`,
  `BUILTIN_SERVER_ARCHITECTURE.md`, `DEPLOYMENT.md` — code-mode/policy/access-control
  + deploy patterns (skim for the engine + policy details D-02/CF-7).

### Toolkit + Code-Mode Surface (the reuse target)

- `crates/pmcp-server-toolkit/src/sql/mod.rs` — `SqlConnector` trait + redaction
  discipline; the analog `HttpConnector` mirrors its shape.
- `crates/pmcp-server-toolkit/src/code_mode.rs` — `SqlCodeExecutor` +
  `code_mode_tools_from_executor`; the precedent for the `HttpExecutor` seam (D-02).
- `crates/pmcp-server-toolkit/src/{config,tools,auth,resources,prompts,builder_ext}.rs`
  — `ServerConfig` (extend with `[backend]`, D-06), `synthesize_from_config_*`,
  `ServerBuilderExt` (the ≤15-line wiring, CF-5).
- `crates/pmcp-code-mode/{Cargo.toml,src/lib.rs,src/executor.rs}` — confirm the
  CURRENT engine (`JsCodeExecutor`/`HttpExecutor`, `js-runtime`/`openapi-code-mode`
  features) and unify script tools + Code Mode on it (D-02).
- `crates/pmcp-sql-server/src/{lib,cli,dispatch,assemble}.rs` + `Cargo.toml` — the
  Shape A binary to structurally copy.
- `cargo-pmcp/src/commands/new.rs` + `cargo-pmcp/src/templates/sql_server.rs` — the
  `--kind sql-server` scaffold to mirror for `--kind openapi-server` (CF-3).

### Requirements, Roadmap & Conventions

- `.planning/ROADMAP.md` §"Phase 90" — goal, depends-on (83/85/86), confirmed
  reference, proposed scope + open questions (now resolved by D-01..D-06).
- `.planning/REQUIREMENTS.md` — v2.2 config-only-MCP-servers milestone value.
- `CLAUDE.md` §"Release & Publish Workflow" — `pmcp-openapi-server` publish slot
  (after the toolkit + connector crates); §"ALWAYS Requirements" + §"PMAT
  Quality-Gate" — coverage matrix + cognitive complexity ≤25.
- `.claude/projects/-Users-guy-Development-mcp-sdk-rust-mcp-sdk/memory/feedback_avoid_docker_pure_rust_lambda.md`
  — pure-Rust drivers, no Docker/testcontainers (wiremock for HTTP, D-04); GCR is
  the only Docker deploy target.
- `.claude/skills/spike-findings-rust-mcp-sdk/` — schema-server architecture +
  dual-surface invariant (auto-loaded).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`pmcp-server-toolkit` (Phase 83)** already abstracts inbound auth, secrets
  (env + AWS SM/SSM, `provider="auto"`), config parse + `${ENV}` expansion, static
  resources/prompts, the `[[tools]]` synthesizer, and code-mode token/HMAC/policy
  machinery — all **reused verbatim**. The phase is ~85% wiring.
- **`pmcp-code-mode 0.5.1`** ships the JS engine + `HttpExecutor` trait SDK-resident
  — no new JS sandbox is written (D-02; research "Don't Hand-Roll").
- **`pmcp-sql-server` (Phase 85)** is the structural template for the binary;
  **`cargo pmcp new --kind sql-server` (Phase 86)** is the template for the scaffold.
- **Reference `HttpClient` / `HttpClientExecutor` / `schema/parser.rs`** are the
  three genuinely-new files to lift (connector, code-mode seam, spec parser).

### Established Patterns

- **`#[serde(deny_unknown_fields)]` strict config** — `[backend]` extends it
  additively (D-06); renames forbidden, additive keys allowed (REF-01 superset).
- **Connector trait + `Arc<dyn _>` dispatch** — `HttpConnector` mirrors
  `SqlConnector`; binary dispatches on `[backend]`.
- **Code-mode executor seam** — `HttpExecutor` impl feeding the SDK engine mirrors
  `SqlCodeExecutor` + `code_mode_tools_from_executor`; the SAME engine also backs
  script tools (D-02).
- **Error redaction** — `HttpConnectorError`/`DispatchError` `Display` names
  operation/status only, never URL/token (research Pitfall 5; test it).
- **Env-gated authentic integration tests** — wiremock default, live network gated.
- **`--test-threads=1`** project CI convention for spawn+replay harnesses.

### Integration Points

- **Root `Cargo.toml` `[workspace.members]`** — add `crates/pmcp-openapi-server`.
- **`[backend] type`/dispatch → `HttpConnector`** — the one new backend seam.
- **`script` vs `path`+`method` detection** — the new tool-registration branch
  (D-01) in the toolkit synthesizer + `assemble.rs`.
- **Same JS engine for script tools + Code Mode** — the unification seam (D-02).
- **`cargo pmcp new` `execute` ← `--kind openapi-server`** — new dispatch (CF-3).
- **wiremock ← spawned HTTP server (london-tube)** — the parity harness (D-04).

</code_context>

<specifics>
## Specific Ideas

- **Tools are promoted scripts (user-stated, central):** a tool is the optimized,
  verified, simplified form of a frequently-used code-mode operation. SQL→SQL
  query, GraphQL→GQL query/mutation, OpenAPI→JS script. The OpenAPI long-tail uses
  a JS subset to describe API calls; promoting a frequent script to a curated tool
  must change nothing about how it runs. Captured as D-01.
- **One JS engine, shared (user-stated, hard requirement):** the same engine
  translates script→API calls in Code Mode and in curated tool definitions —
  including chaining one call's output into the next, and `filter`/`map` over an
  array from one call into the next step. Captured as D-02.
- **Spec useful in both aspects (user-stated):** the OpenAPI schema informs both
  the curated-tool/script surface and the code-mode long-tail; `--spec` optional at
  runtime but valuable wherever present. Captured as D-03.
- **london-tube as parity target (user choice):** exercises api_key query-param
  auth — the new auth path worth proving. Captured as D-04.

</specifics>

<deferred>
## Deferred Ideas

- **`lichess` as a second demo instance** — bearer/optional auth; a nice secondary
  showcase but not the parity target. Add additively if a second demo is wanted.
- **Live-network parity replay** against api.tfl.gov.uk — env-gated, not default CI.
- **GraphQL built-in server** — the third backend sibling (tool = GQL
  query/mutation per the D-01 model); its own phase/milestone.
- **Full live integration tests for every auth variant** — all five are lifted
  (D-05) but london-tube only exercises `api_key`; `basic`/`oauth2`/`passthrough`
  get unit coverage now, live tests deferred until a backend needs them.
- **Non-OpenAPI `--kind` scaffold backends / broad cargo-pmcp README rewrite** —
  Phase 89 (DOCS) owns the broad rewrite; only the `--kind openapi-server` surface
  is documented here.

### Reviewed Todos (not folded)

- **`2026-05-18-ship-sqlite-from-config-example-as-phase-86-shape-b-c-dogfood.md`**
  — a SQLite/Shape-B-C dogfood item already owned and addressed by **Phase 86**;
  SQL-specific and not relevant to the OpenAPI built-in. Reviewed, not folded.

</deferred>

---

*Phase: 90-openapi-built-in-server*
*Context gathered: 2026-05-29*
