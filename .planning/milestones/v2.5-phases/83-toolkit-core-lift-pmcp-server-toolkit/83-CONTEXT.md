# Phase 83: Toolkit Core Lift (`pmcp-server-toolkit`) - Context

**Gathered:** 2026-05-18
**Status:** Ready for planning

<domain>
## Phase Boundary

Lift the proto-SDK shape already extracted at `pmcp-run/built-in/shared/mcp-server-common/` (~2.2k LoC) plus the `pmcp-code-mode` integration shape into a new public workspace crate **`crates/pmcp-server-toolkit/`** that is publishable to crates.io and consumable by external developers as a runtime library for config-driven MCP servers.

In scope:

- New workspace crate `crates/pmcp-server-toolkit/` slotted in publish order between `pmcp` and `mcp-tester`
- Public API surface: `AuthProvider`, `SecretsProvider`, `StaticResourceHandler`, `StaticPromptHandler`, HMAC token machinery (TKIT-02..06)
- New code: `[[tools]]` → `ToolInfo` synthesizer (TKIT-07) + `[code_mode]` config wiring to `pmcp-code-mode::CodeExecutor` (TKIT-09) + curated-table-aware code-mode prompt assembly (TKIT-10)
- `SqlConnector` trait + `Dialect` enum **stubs** in toolkit core (per spike 005; real impls land in Phase 84)
- Re-export shim diff for `pmcp-run/built-in/shared/mcp-server-common/` so existing pmcp-run backend cores keep building without code changes (SC-5)
- Full ALWAYS test coverage per `CLAUDE.md`: unit + property + doctest + integration + fuzz

Out of scope (other phases own these):

- Per-backend SQL connector crates (Postgres / MySQL / Athena), SQLite feature impl → **Phase 84**
- `pmcp-sql-server` pure-config binary + reference parity → **Phase 85**
- Scaffolding, 15-line library example, deploy → **Phase 86**
- Type 2 authoring Skills server (`pmcp-config-helper`) → **Phase 87**
- `crates/pmcp-server` dogfood rewrite → **Phase 88**
- Book chapter, course tutorial, migration recipe → **Phase 89**
- OpenAPI code-mode (`openapi-code-mode` / `js-runtime` / `mcp-code-mode` features) → deferred (Phase 3 OpenAPI lift, gated by spike 007)
- DynamoDB config/AttributeValue features (`ddb`, `dynamo-config`) → pmcp-run-specific, not generalised

</domain>

<decisions>
## Implementation Decisions

### Lift Mechanics & Cross-Repo Coordination

- **D-01:** **Toolkit publish + pmcp-run re-export shim, incremental cutover.** Phase 83 publishes `pmcp-server-toolkit` AND captures a "pure re-export shim" diff for `pmcp-run/built-in/shared/mcp-server-common/lib.rs` (essentially `pub use pmcp_server_toolkit::*;` plus feature-gated AVP/DDB re-exports) as a P83 artifact. The operator submits that shim PR to the pmcp-run repo after the toolkit publishes. The three pmcp-run backend cores' direct path-dep swaps are tracked separately and don't block P83 verification.
- **D-02:** **Shim shape = pure re-export, lib.rs only.** The shim replaces every `.rs` file in `mcp-server-common/src/` with a single `lib.rs` that does `pub use pmcp_server_toolkit::*` with feature-gated re-exports for AVP and DDB types where they apply. Drops mcp-server-common's direct deps in favor of pulling them transitively through the toolkit. Smallest diff, zero behavior drift, easy to delete entirely once cores swap their imports.
- **D-03:** **SC-5 verification via in-toolkit smoke test.** P83 ships `crates/pmcp-server-toolkit/tests/backend_core_smoke.rs` that constructs an `AuthProvider`, `SecretsProvider`, `StaticResourceHandler`, `StaticPromptHandler`, and HMAC token from the toolkit's public API — mirroring the construction surface each pmcp-run backend core uses today. Proves the toolkit covers their import surface without making P83 verification depend on cloning the pmcp-run repo in CI.
- **D-04:** **Shim diff lives in `.planning/phases/83-.../shim-pmcp-run-shared.md`** as a copy-paste-ready apply-bundle (lib.rs content + Cargo.toml diff + apply instructions). Operator handoff is documented in STATE.md / HANDOFF.json. Cross-repo work stays visible inside P83's manifest without making P83 own the second repo's merge.

### pmcp Dependency Direction & Release Cadence

- **D-05:** **Workspace-version trick for both `pmcp` and `pmcp-code-mode`.** Toolkit's `Cargo.toml` uses `pmcp = { version = "2.x", path = "../.." }` and `pmcp-code-mode = { version = "0.4.x", path = "../pmcp-code-mode" }`. Cargo uses the path locally; `cargo publish` emits the version constraint. Preserves independent release cadence (spike-validated headline benefit) AND lets local dev consume in-tree changes immediately.
- **D-06:** **`pmcp-code-mode` is feature-gated.** Toolkit's `code-mode` feature pulls in the `pmcp-code-mode` dep + AVP/Cedar policy types. `default = ["code-mode"]` (see D-12) since `[code_mode]` is core to v2.2's value prop — but it can be opted out for users who want a pure-curated-tools server.
- **D-07:** **Initial published version: `0.1.0`.** Fresh 0.x crate, signals pre-1.0 API may evolve as DX matures across Phases 84–89. Matches the pattern of `mcp-tester@0.5.0` and `mcp-preview@0.3.0` entering the workspace.
- **D-08:** **Publish order slot: after `pmcp`, before `mcp-tester`.** New canonical order: `pmcp-widget-utils → pmcp → pmcp-server-toolkit → mcp-tester → mcp-preview → cargo-pmcp`. CLAUDE.md §"Release & Publish Workflow" needs a one-line edit inserting toolkit in this slot. Phase 83 emits that edit as part of the release-workflow-prep task.
- **D-09 (rationale recorded):** Toolkit is a **runtime library** — linked into deployed MCP server binaries (Lambda, local, Cloud Run via Shape A/B/C). It cannot be fused with `cargo-pmcp` (a CLI binary / cargo subcommand) because: (a) Lambda zips need minimal library deps not CLI deps like clap + AWS account-management SDKs; (b) downstream devs writing Shape C ≤15-line `main.rs` need a library not a binary; (c) `pmcp-run` backend cores already depend on the library shape today. cargo-pmcp CAN consume the toolkit (e.g., `cargo pmcp validate-config` in Phase 86) but the runtime library must be independently shippable.

### `[[tools]]` Synthesizer & Code-Mode Wiring API

- **D-10:** **Synthesizer ships BOTH a low-level fn AND a builder extension.** Low-level: `pmcp_server_toolkit::tools::synthesize_from_config(&config) -> Result<Vec<(String, ToolInfo, Arc<dyn ToolHandler>)>>`. Builder extension trait: `pmcp_server_toolkit::ServerBuilderExt::tools_from_config(self, &config) -> Self` that internally calls the low-level fn and wires each handler via Phase 82's `tool_arc`. Composable for power users; one-line for the common Shape A/C case.
- **D-11:** **Code-mode wires via `.code_mode_from_config(&config)` builder extension.** Reads the `[code_mode]` block, constructs a `CodeExecutor` with the right policy (allow_writes/allow_deletes/allow_ddl/require_limit/max_limit/blocked_tables/sensitive_columns/auto_approve_levels/token_ttl_seconds/token_secret + `[code_mode.limits]`), and registers the `validate_code` / `execute_code` tool pair plus HMAC token machinery. Symmetric with `tools_from_config()`. Pairs with a low-level `code_mode::executor_from_config(&config) -> Result<CodeExecutor>` for power users who need the executor before registration (e.g., to wrap in custom middleware).
- **D-12:** **TKIT-10 prompt assembly lives in P83; SqlConnector implementations land in P84.** Ship `pmcp_server_toolkit::code_mode::assemble_code_mode_prompt(connector: &dyn SqlConnector, config: &ServerConfig) -> String` that calls `connector.schema_text()` (trait method declared in toolkit core per spike 005) and folds in curated descriptions from `[[database.tables]]` entries. P83 tests against a stub `SqlConnector` impl that returns canned schema_text. Phase 84's per-backend crates plug in real connectors and the assembly fn just works.
- **D-13:** **Single `ServerConfig` struct, `#[serde(deny_unknown_fields)]` on all sections.** Top-level type lives at `pmcp_server_toolkit::config::ServerConfig` and parses the entire config.toml in one shot (server/metadata/database/[[database.tables]]/code_mode/[code_mode.limits]/[[tools]]/[[tools.parameters]]/[tools.annotations]/[[prompts]]/etc.). Strict mode catches typos like `auto_aprove_levels` at parse time. REF-01 superset-of-pmcp-run shape is enforced by ADDING fields to the struct (not by loosening `deny_unknown_fields`); renames are forbidden.

### Public API Surface & Feature Flag Matrix

- **D-14:** **Slim MVP feature set in 0.1.0.** Default features: `["code-mode"]`. Optional features: `aws` (Secrets Manager + SSM via aws-sdk-secretsmanager + aws-sdk-ssm), `avp` (Cedar/Amazon Verified Permissions code-mode policy via pmcp-code-mode/avp + aws-sdk-verifiedpermissions + chrono), `input-validation` (jsonschema for tool arg validation), `sqlite` (rusqlite bundled, for Phase 84 dev/CI). **Dropped vs `mcp-server-common`:** `openapi-code-mode`, `js-runtime`, `mcp-code-mode` (Phase 3 OpenAPI territory — add later if/when spike 007 unblocks); `ddb`, `dynamo-config` (pmcp-run-specific concerns, don't generalize to public toolkit users).
- **D-15:** **Flat module set with crate-root re-exports.** Public modules: `pmcp_server_toolkit::{auth, secrets, config, prompts, resources, code_mode, tools, sql}`. Headline types re-exported at crate root for ergonomic imports: `use pmcp_server_toolkit::{AuthProvider, SecretsProvider, ServerConfig, StaticResourceHandler, StaticPromptHandler, SqlConnector, Dialect}`. Inherits mcp-server-common's shape — smallest cognitive delta, lowest re-export-shim translation cost.
- **D-16:** **Code-mode types re-exported through `toolkit::code_mode`.** `pmcp_server_toolkit::code_mode` re-exports `pmcp_code_mode::{CodeExecutor, TokenSecret, NoopPolicyEvaluator, AvpPolicyEvaluator}` (the last gated behind the `avp` feature). Users get them via `use pmcp_server_toolkit::code_mode::*` — single dep in their Cargo.toml. Keeps Shape C ≤15-line main.rs target reachable. Toolkit becomes the official public surface for code-mode wiring.
- **D-17:** **TEST-02/TEST-03 hit the full ALWAYS shape from CLAUDE.md.** Coverage matrix: (1) unit tests ≥80% coverage per module, (2) property tests on config.toml round-trip + ToolInfo synthesis (every `[[tools]]` entry → valid ToolInfo with all `[tools.annotations]` + `[[tools.parameters]]` preserved), (3) doctests on every public type/fn, (4) integration test that parses **all three reference servers' config.tomls** (`open-images`, `imdb`, `msr-vtt`) into ToolInfo vectors as the SC-2 verifier, (5) fuzz target on the toolkit config parser (extends Phase 77's `pmcp_config_toml_parser` — reuse, do not duplicate). CLAUDE.md "ALWAYS Requirements for New Features" treats fuzz as non-optional in the originating phase.

### Claude's Discretion

- Exact name of the `ServerBuilderExt` trait, the synthesizer's error type shape (`thiserror`-based per pmcp convention), and the doctest-friendly `MockSqlConnector` used internally for TKIT-10 assembly tests. Planner/researcher resolve these from existing pmcp patterns.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Spike Findings (Primary)

- `.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md` — implementation blueprint, ordering, requirements baseline
- `.claude/skills/spike-findings-rust-mcp-sdk/references/schema-server-architecture.md` — proto-SDK already extracted; lift PROMOTES, doesn't rewrite; per-backend executors diverge semantically (no single `SchemaServer<S,C>` trait)
- `.claude/skills/spike-findings-rust-mcp-sdk/references/schema-server-sql-dialects.md` — `SqlConnector` 3-method trait + `Dialect` 4-variant enum + 2 free helpers (stubs land in toolkit core in P83)
- `.planning/spikes/004-schema-server-thin-slice-sql/` — inline toolkit slice with 12-line user surface (validates Shape C target)
- `.planning/spikes/005-multi-dialect-sql-connector/` — trait shape that toolkit core declares

### Lift Source (Verbatim Promotion)

- `pmcp-run/built-in/shared/mcp-server-common/src/lib.rs` — current module set: auth, config, error, prompts, resources, secrets, tools, code_mode (feature), ddb (feature)
- `pmcp-run/built-in/shared/mcp-server-common/Cargo.toml` — current 9-feature matrix; P83 ships a 5-feature slim variant (see D-14)
- `pmcp-run/built-in/shared/mcp-server-common/src/secrets.rs` (701 LoC), `resources.rs` (333 LoC), `prompts.rs` (285 LoC), `auth.rs` (~600 LoC) — files being lifted

### Reference Server Configs (SC-2 superset verification anchor)

- `pmcp-run/built-in/sql-api/servers/open-images/config.toml` — Athena backend, 394 lines, exemplifies `[[database.tables]]` + `[code_mode]` + `[[tools]]` superset
- `pmcp-run/built-in/sql-api/servers/imdb/config.toml` — second reference for parity
- `pmcp-run/built-in/sql-api/servers/msr-vtt/config.toml` — third reference for parity

### Requirements & Roadmap

- `.planning/REQUIREMENTS.md` §"Toolkit Core" — TKIT-01..10 full text; TEST-02 + TEST-03 toolkit-coverage requirements
- `.planning/ROADMAP.md` §"Phase 83" — goal, depends-on (Phase 82), 5 success criteria (SC-1..5)
- `.planning/phases/82-builder-dx-prerequisites/82-VERIFICATION.md` — confirms `tool_arc` / `prompt_arc` / `get_tool` / `get_prompt` public on `pmcp::ServerBuilder` (P83 builder-extension API depends on these)

### Workspace & Release Conventions

- `Cargo.toml` (root) §`[workspace]` lines 540–543 — current workspace members list; toolkit insertion point
- `CLAUDE.md` §"Release & Publish Workflow" — current publish order (widget-utils → pmcp → mcp-tester → mcp-preview → cargo-pmcp); P83 inserts toolkit between pmcp and mcp-tester (D-08)
- `CLAUDE.md` §"ALWAYS Requirements for New Features" — defines the test-coverage shape D-17 commits to
- `crates/pmcp-code-mode/` — published 0.4.x crate; toolkit re-exports CodeExecutor / TokenSecret / NoopPolicyEvaluator / AvpPolicyEvaluator

### Memory & Conventions

- `.claude/projects/-Users-guy-Development-mcp-sdk-rust-mcp-sdk/memory/feedback_avoid_docker_pure_rust_lambda.md` — pure-Rust drivers only, no Docker/testcontainers (constrains SqlConnector stub design and Phase 84 follow-ups)
- `.claude/projects/-Users-guy-Development-mcp-sdk-rust-mcp-sdk/memory/feedback_v2_cleanup.md` — during the v2.x breaking-change window, consolidate aggressively (informs feature-set trimming in D-14)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`pmcp-run/built-in/shared/mcp-server-common/`** (~2.2k LoC) — the ENTIRE source of the lift. P83 promotes this to `crates/pmcp-server-toolkit/` largely as-is (D-01).
- **`crates/pmcp-code-mode`** (published 0.4.x) — `CodeExecutor`, `TokenSecret`, `NoopPolicyEvaluator`, `AvpPolicyEvaluator` — toolkit re-exports through `code_mode` module (D-16) and wires via builder extension (D-11).
- **`pmcp::ServerBuilder::{tool_arc, prompt_arc, get_tool, get_prompt}`** — landed in Phase 82. Builder extension methods (D-10, D-11) use these as the registration substrate; no need for the 20-line delegating wrapper shim.
- **Phase 77 `pmcp_config_toml_parser` fuzz target** — extends to cover the toolkit config types (D-17 reuse, not duplicate).

### Established Patterns

- **Workspace-version dep pattern** — used by existing workspace crates like `crates/mcp-tester` and `crates/mcp-preview` (`pmcp = { version = "...", path = "..." }`). D-05 inherits this.
- **Cargo features matrix** — `mcp-server-common` already follows the pattern `default = []` + feature-gated AWS / code-mode / observability tiers. Toolkit slims (D-14) but inherits the pattern.
- **`#[serde(deny_unknown_fields)]`** — pmcp config types in `src/types/protocol.rs` already use this for strict parsing. D-13 inherits.
- **Builder extension traits** — pattern used in PMCP for things like the `WorkflowBuilder` family. D-10 / D-11 inherit.
- **PMAT cognitive-complexity ≤25** — required by Phase 75 CI gate. Synthesizer + assembly fn must comply or carry annotated `#[allow]`.

### Integration Points

- **`pmcp::ServerBuilder`** — toolkit's builder extension trait extends this surface. Cannot modify pmcp itself; works via trait impl on the existing type.
- **`Cargo.toml` root `[workspace.members]`** — toolkit gets inserted (line 541). Publish-order update in CLAUDE.md.
- **`pmcp-run/built-in/shared/mcp-server-common/`** — shim diff (D-04) lands here as an external artifact (not committed by P83 itself; captured for operator handoff).
- **`docs.rs` metadata in root Cargo.toml** — toolkit gets its own `[package.metadata.docs.rs]` block enabling the right feature flags for documentation.

</code_context>

<specifics>
## Specific Ideas

- **Shape C ≤15-line main.rs target** — every DX decision (D-10 builder extension, D-11 code_mode builder extension, D-16 code-mode re-exports) is evaluated against this constraint. Asserted in-binary in P85 / P86 examples; informs P83 API ergonomics.
- **REF-01 superset invariant** — `pmcp-run/built-in/sql-api/servers/*/config.toml` files MUST parse against the toolkit's `ServerConfig` without modification. Additive new keys allowed; renames forbidden. Enforced via D-13 strict parsing + D-17 integration test against all three reference configs.
- **Lambda-deployable, pure-Rust** — no Docker, no testcontainers (per feedback memory). Constrains every future connector crate AND the test/fuzz infrastructure P83 establishes.

</specifics>

<deferred>
## Deferred Ideas

- **OpenAPI code-mode features** (`openapi-code-mode`, `js-runtime`, `mcp-code-mode`) — Phase 3 OpenAPI lift, gated by spike 007 (`openapi-auth-policy-pluggability`). Not in P83.
- **DynamoDB features** (`ddb`, `dynamo-config`) — pmcp-run-specific concerns (team-mcp, durable-agent). If a generalised pattern emerges, fold back into the toolkit in a later phase.
- **`pmcp-config-helper` MCP server** (Type 2 authoring skills) — Phase 87 owns this; P83 just publishes the toolkit that helper teaches users to configure.
- **`pmcp-sql-server` pure-config binary** — Phase 85 builds this on top of the toolkit + at least one Phase 84 connector.
- **Per-backend SQL connector crates** (`pmcp-toolkit-postgres`, `pmcp-toolkit-mysql`, `pmcp-toolkit-athena`) and the SQLite feature impl — Phase 84. P83 declares the trait stub only.
- **CLAUDE.md §"Release & Publish Workflow" edit** — adding `pmcp-server-toolkit` to the publish order list. Captured here as a P83 task; the actual edit is part of the release-workflow-prep step.

</deferred>

---

*Phase: 83-toolkit-core-lift-pmcp-server-toolkit*
*Context gathered: 2026-05-18*
