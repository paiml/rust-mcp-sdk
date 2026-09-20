# Phase 83: Toolkit Core Lift (`pmcp-server-toolkit`) - Research

**Researched:** 2026-05-18
**Domain:** Rust crate-creation + proto-SDK lift + config-driven MCP server runtime library
**Confidence:** HIGH (CONTEXT.md decisions are locked; existing workspace patterns + spike findings + Phase 82 deliverables fully constrain the design surface)

## Summary

Phase 83 promotes the `mcp-server-common` proto-SDK shape (~2.2k LoC at `pmcp-run/built-in/shared/`) into a new workspace crate `crates/pmcp-server-toolkit/` and adds three pieces of net-new code: (a) a `[[tools]]`→`ToolInfo` synthesizer that turns curated config entries into complete pmcp tool definitions with zero per-tool Rust, (b) a `[code_mode]` → `pmcp_code_mode::CodeExecutor` config bridge with `ServerBuilder` extension methods, and (c) a curated-table-aware code-mode prompt assembler that depends on a `SqlConnector::schema_text()` trait stub which Phase 84 will implement. CONTEXT.md locks the lift mechanics (D-01..D-04: incremental cutover with an in-toolkit smoke test + a separately-applied shim diff), pmcp dependency direction (D-05..D-09: workspace-version trick, `code-mode` feature-gated default, `0.1.0` initial publish, slotted after `pmcp` and before `mcp-tester`), the synthesizer/builder-extension API (D-10..D-13: low-level fn + builder extension, single `ServerConfig` with `#[serde(deny_unknown_fields)]`), and feature/module layout (D-14..D-17: slim 5-feature MVP, flat 8-module set, crate-root re-exports of headline types, full ALWAYS test coverage). Everything in this research either confirms a locked decision or fills in the Claude's-discretion gap (error-type shapes, internal `MockSqlConnector`, exact synthesizer signature).

The lift is mechanically simple but has high blast radius: 12 requirement IDs are in scope, two of them (TKIT-08 cross-repo backend-core migration; TKIT-10 prompt assembly that depends on a Phase-84 trait) interlock with adjacent phases. The decisions cleanly defuse both: TKIT-08 is satisfied by an in-toolkit smoke test mirroring the pmcp-run backend cores' construction surface (D-03), and TKIT-10 ships against a stubbed `SqlConnector` impl that returns canned `schema_text` (D-12). The phase is large but coherent — recommend planning as **one phase with ~8 plans** (sequential within the phase, parallelisable around the workspace-Cargo.toml edit). See §"Phase-Split Risk Assessment" for the rationale.

**Primary recommendation:** Follow CONTEXT.md decisions D-01..D-17 verbatim. Net-new code lives in `tools.rs`, `code_mode.rs`, and `sql/mod.rs` (the connector trait stub); everything else is a verbatim promotion from `mcp-run/built-in/shared/mcp-server-common/src/`. Use the workspace-version dep pattern proven by `crates/mcp-tester` (`pmcp = { version = "2.8.x", path = "../.." }`). The ALWAYS test shape is satisfied by a single fuzz target (`pmcp_server_toolkit_config_parser` — independent of cargo-pmcp's Phase 77 target since the schemas differ) + property tests on round-trip + integration test against all three reference config.tomls + doctests on every public type. Plan as 8 plans: scaffold/Cargo, config-parse, auth+secrets+static surfaces lift, HMAC token re-exports + smoke, ToolInfo synthesizer, code-mode wiring, prompt assembler + SqlConnector stub, ALWAYS gates + verification.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Lift Mechanics & Cross-Repo Coordination**

- **D-01:** **Toolkit publish + pmcp-run re-export shim, incremental cutover.** Phase 83 publishes `pmcp-server-toolkit` AND captures a "pure re-export shim" diff for `pmcp-run/built-in/shared/mcp-server-common/lib.rs` (essentially `pub use pmcp_server_toolkit::*;` plus feature-gated AVP/DDB re-exports) as a P83 artifact. The operator submits that shim PR to the pmcp-run repo after the toolkit publishes. The three pmcp-run backend cores' direct path-dep swaps are tracked separately and don't block P83 verification.
- **D-02:** **Shim shape = pure re-export, lib.rs only.** The shim replaces every `.rs` file in `mcp-server-common/src/` with a single `lib.rs` that does `pub use pmcp_server_toolkit::*` with feature-gated re-exports for AVP and DDB types where they apply. Drops mcp-server-common's direct deps in favor of pulling them transitively through the toolkit. Smallest diff, zero behavior drift, easy to delete entirely once cores swap their imports.
- **D-03:** **SC-5 verification via in-toolkit smoke test.** P83 ships `crates/pmcp-server-toolkit/tests/backend_core_smoke.rs` that constructs an `AuthProvider`, `SecretsProvider`, `StaticResourceHandler`, `StaticPromptHandler`, and HMAC token from the toolkit's public API — mirroring the construction surface each pmcp-run backend core uses today. Proves the toolkit covers their import surface without making P83 verification depend on cloning the pmcp-run repo in CI.
- **D-04:** **Shim diff lives in `.planning/phases/83-.../shim-pmcp-run-shared.md`** as a copy-paste-ready apply-bundle (lib.rs content + Cargo.toml diff + apply instructions). Operator handoff is documented in STATE.md / HANDOFF.json.

**pmcp Dependency Direction & Release Cadence**

- **D-05:** **Workspace-version trick for both `pmcp` and `pmcp-code-mode`.** Toolkit's `Cargo.toml` uses `pmcp = { version = "2.x", path = "../.." }` and `pmcp-code-mode = { version = "0.5.x", path = "../pmcp-code-mode" }`. Cargo uses the path locally; `cargo publish` emits the version constraint.
- **D-06:** **`pmcp-code-mode` is feature-gated.** Toolkit's `code-mode` feature pulls in the `pmcp-code-mode` dep + AVP/Cedar policy types. `default = ["code-mode"]` (D-12).
- **D-07:** **Initial published version: `0.1.0`.** Fresh 0.x crate.
- **D-08:** **Publish order slot: after `pmcp`, before `mcp-tester`.** New canonical order: `pmcp-widget-utils → pmcp → pmcp-server-toolkit → mcp-tester → mcp-preview → cargo-pmcp`.
- **D-09 (rationale):** Toolkit is a **runtime library**, not fused with `cargo-pmcp` (CLI binary).

**`[[tools]]` Synthesizer & Code-Mode Wiring API**

- **D-10:** **Synthesizer ships BOTH a low-level fn AND a builder extension.** Low-level: `pmcp_server_toolkit::tools::synthesize_from_config(&config) -> Result<Vec<(String, ToolInfo, Arc<dyn ToolHandler>)>>`. Builder extension trait: `pmcp_server_toolkit::ServerBuilderExt::tools_from_config(self, &config) -> Self`.
- **D-11:** **Code-mode wires via `.code_mode_from_config(&config)` builder extension.** Symmetric with `tools_from_config()`. Pairs with a low-level `code_mode::executor_from_config(&config) -> Result<CodeExecutor>`.
- **D-12:** **TKIT-10 prompt assembly lives in P83; SqlConnector implementations land in P84.** Ship `pmcp_server_toolkit::code_mode::assemble_code_mode_prompt(connector: &dyn SqlConnector, config: &ServerConfig) -> String`. P83 tests against a stub `SqlConnector` impl that returns canned schema_text.
- **D-13:** **Single `ServerConfig` struct, `#[serde(deny_unknown_fields)]` on all sections.** Top-level type at `pmcp_server_toolkit::config::ServerConfig`. REF-01 superset is enforced by ADDING fields, not by loosening `deny_unknown_fields`; renames forbidden.

**Public API Surface & Feature Flag Matrix**

- **D-14:** **Slim MVP feature set in 0.1.0.** Default features: `["code-mode"]`. Optional: `aws` (Secrets Manager + SSM), `avp` (Cedar/AVP via pmcp-code-mode/avp), `input-validation` (jsonschema), `sqlite` (rusqlite bundled). **Dropped vs `mcp-server-common`:** `openapi-code-mode`, `js-runtime`, `mcp-code-mode`, `ddb`, `dynamo-config`.
- **D-15:** **Flat module set with crate-root re-exports.** Public modules: `pmcp_server_toolkit::{auth, secrets, config, prompts, resources, code_mode, tools, sql}`. Headline types re-exported at crate root.
- **D-16:** **Code-mode types re-exported through `toolkit::code_mode`.** `pmcp_server_toolkit::code_mode` re-exports `pmcp_code_mode::{CodeExecutor, TokenSecret, NoopPolicyEvaluator, AvpPolicyEvaluator}` (last gated behind `avp` feature).
- **D-17:** **TEST-02/TEST-03 hit the full ALWAYS shape.** Unit tests ≥80% coverage per module, property tests on config.toml round-trip + ToolInfo synthesis, doctests on every public type/fn, integration test parsing all three reference servers' config.tomls, fuzz target on toolkit config parser.

### Claude's Discretion

- Exact name of the `ServerBuilderExt` trait
- Synthesizer's error type shape (`thiserror`-based per pmcp convention)
- Doctest-friendly `MockSqlConnector` used internally for TKIT-10 assembly tests

### Deferred Ideas (OUT OF SCOPE)

- **OpenAPI code-mode features** (`openapi-code-mode`, `js-runtime`, `mcp-code-mode`) — Phase 3 OpenAPI lift, gated by spike 007.
- **DynamoDB features** (`ddb`, `dynamo-config`) — pmcp-run-specific.
- **`pmcp-config-helper` MCP server** (Type 2 authoring skills) — Phase 87.
- **`pmcp-sql-server` pure-config binary** — Phase 85.
- **Per-backend SQL connector crates** (`pmcp-toolkit-postgres`, `-mysql`, `-athena`) and SQLite feature impl — Phase 84. P83 declares the trait stub only.
- **CLAUDE.md §"Release & Publish Workflow" edit** — adding `pmcp-server-toolkit` to publish order. Captured as a P83 task; actual edit is part of release-workflow-prep step.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TKIT-01 | `crates/pmcp-server-toolkit/` exists, builds, publishable | §Crate Skeleton, §Workspace Slot — slots between pmcp and mcp-tester at workspace member line 541; uses mcp-tester's `path = "../../"` + `version = "..."` pattern |
| TKIT-02 | `AuthProvider` trait + at least one concrete impl in public API | §Existing Analogs — pmcp ships `pmcp::AuthProvider` trait at `src/server/auth/traits.rs:450`. Toolkit lifts mcp-server-common's auth shape (StaticAuthProvider / BearerAuthProvider — verify in lift) which re-uses `pmcp::AuthProvider` trait, not redefines |
| TKIT-03 | `SecretsProvider` trait + concrete impl in public API | §Existing Analogs — no pmcp equivalent exists; this is a verbatim lift from `mcp-server-common/src/secrets.rs` (701 LoC). Trait + EnvSecrets / SsmSecrets / SecretsManager impls (last two behind `aws` feature) |
| TKIT-04 | `StaticResourceHandler` constructible from config | §Existing Analogs — pmcp's `ResourceHandler` trait at `src/server/mod.rs:256`; `src/server/skills.rs` shows the `SkillsResourceHandler` pattern using indexmap + Content::resource_with_text. Lift verbatim from `mcp-server-common/src/resources.rs` (333 LoC) — already implements `pmcp::ResourceHandler` |
| TKIT-05 | `StaticPromptHandler` constructible from config | §Existing Analogs — lift verbatim from `mcp-server-common/src/prompts.rs` (285 LoC). Implements `pmcp::PromptHandler` trait from `src/server/mod.rs:238` |
| TKIT-06 | HMAC token machinery (sign + verify, code-hash binding) in public API + integrated with pmcp-code-mode | §HMAC Token Helpers — re-export `pmcp_code_mode::{ApprovalToken, HmacTokenGenerator, TokenGenerator, TokenSecret, hash_code, canonicalize_code, compute_context_hash}` from `pmcp_server_toolkit::code_mode`. Verified at `crates/pmcp-code-mode/src/lib.rs:160-163`. NO duplicate impl in toolkit. |
| TKIT-07 | `ToolInfo` synthesizer reads `[[tools]]`, produces ToolInfo, zero per-tool Rust | §ToolInfo Synthesizer — `pmcp::types::ToolInfo` at `src/types/tools.rs:176` has `name`, `description`, `input_schema: Value`, `annotations: Option<ToolAnnotations>`. `ToolAnnotations` at `tools.rs:20` has `read_only_hint`, `destructive_hint`, `idempotent_hint`, `open_world_hint`. Synthesizer signature: `synthesize_from_config(&ServerConfig) -> Result<Vec<(String, ToolInfo, Arc<dyn ToolHandler>)>>` (D-10) |
| TKIT-08 | All three pmcp-run backend cores swap path-deps for crates.io `pmcp-server-toolkit` | §Backend-Core Migration — D-03 reframes: verification via in-toolkit smoke test `tests/backend_core_smoke.rs` reproducing the construction surface. Actual cross-repo PR captured as shim diff artifact per D-04 |
| TKIT-09 | `[code_mode]` block + `[code_mode.limits]` wire into pmcp-code-mode CodeExecutor, zero per-server Rust glue | §Code-Mode Wiring — `pmcp_code_mode::CodeExecutor` trait at `crates/pmcp-code-mode/src/code_executor.rs:55` (always available, no feature gate). `pmcp_code_mode::CodeModeConfig` at `crates/pmcp-code-mode/src/config.rs:95` already accepts most `[code_mode]` fields with `alias = "..."` for the unprefixed forms (see lines 189–236). Toolkit wraps with `code_mode::executor_from_config(&ServerConfig) -> Result<CodeExecutor>` + builder extension `.code_mode_from_config()` |
| TKIT-10 | Code-mode prompt body combines build_code_mode_prompt (CONN-04, Phase 84) with `[[database.tables]]` descriptions | §Prompt-Body Assembler — define `SqlConnector` trait stub + `Dialect` enum stub in toolkit core per spike 005 (referenced by canonical_refs). `assemble_code_mode_prompt(connector: &dyn SqlConnector, config: &ServerConfig) -> String` calls `connector.schema_text()` and folds in `config.database.tables[*].description`. P83 tests against `MockSqlConnector` returning canned schema_text |
| TEST-02 | Toolkit core unit + property tests covering placeholder translation invariants, code-mode prompt assembly, ToolInfo synthesis | §Validation Architecture — property tests under `tests/` using `proptest = "1.7"` (already a workspace dev-dep); ToolInfo round-trip + every `[[tools]]` produces non-empty schema |
| TEST-03 | Public API doctest coverage for `pmcp-server-toolkit` (all public types + helpers compile and run as rust,no_run or rust doctests) | §Validation Architecture — doctests on every `pub fn` / `pub struct` / `pub trait`; enforce via `cargo test --doc -p pmcp-server-toolkit --all-features` |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

The planner must verify all plans comply with these directives:

- **Toyota Way Zero Tolerance** — `make quality-gate` must pass before any commit (fmt, clippy, build, tests, audit). Phase 75 CI gate blocks PRs with new cognitive complexity > 25; hard cap 50 with annotated `#[allow]`.
- **ALWAYS Requirements** — Every new feature MUST ship unit + property + fuzz + example + integration + doctest (CLAUDE.md §"ALWAYS Requirements for New Features"). Maps to D-17.
- **Contract-First Development** — New crate's public API surface must have a contract YAML entry (extend `contracts/binding.yaml` with `pmcp-server-toolkit` rows OR create a new `contracts/toolkit-v1.yaml`). Run `pmat comply check` before commit.
- **Pre-commit Hook** — automatically runs Toyota Way quality checks; cannot commit without passing.
- **PMAT proxy enforcement** — All code changes MUST go through `pmat mcp-server --enable-quality-proxy` (CLAUDE.md §"PMAT Quality-Gate Proxy Mode"). Use `quality_proxy` MCP tool for write/edit/append.
- **No bare `cargo test <pattern>`** — Use `make test-fuzz`, `make test-property`, `make test-unit`, `make test-examples`, `make test-integration`, `make test-doc` targets.
- **No Docker / no testcontainers** — Pure-Rust Lambda deployment target; constrains every connector test (informs `MockSqlConnector` design for TKIT-10).
- **Release & Publish Workflow** — D-08 inserts `pmcp-server-toolkit` between `pmcp` and `mcp-tester`. CLAUDE.md §"Release & Publish Workflow" lists current order; phase ships a one-line edit.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Config parsing (`ServerConfig::from_toml`) | Toolkit library (`pmcp-server-toolkit/src/config.rs`) | — | Single entrypoint per spike 003; pure parse, no I/O |
| Auth + secrets + static resources/prompts | Toolkit library | pmcp core (uses `pmcp::AuthProvider`, `pmcp::ResourceHandler`, `pmcp::PromptHandler` traits) | Verbatim lift from mcp-server-common; toolkit provides ready-made impls of pmcp traits |
| HMAC token sign/verify | pmcp-code-mode (existing) | Toolkit (re-export only via `code_mode::*`) | Already shipped; D-16 forbids duplication |
| `[[tools]]` → `ToolInfo` synthesis | Toolkit library (`tools.rs`) | pmcp core (`pmcp::types::ToolInfo` + `pmcp::server::ToolHandler` trait) | Net-new code; produces handlers registered via Phase 82's `tool_arc` |
| `[code_mode]` → `CodeExecutor` wiring | Toolkit library (`code_mode.rs`) | pmcp-code-mode (`CodeExecutor` trait + `CodeModeConfig` struct) | Net-new code; thin glue, no validation logic duplicated |
| Code-mode prompt assembly | Toolkit library (`code_mode::assemble_code_mode_prompt`) | Phase 84 (`SqlConnector::schema_text` impls) | Net-new code; stubbed trait in P83, real impls in P84 |
| SqlConnector trait declaration | Toolkit library (`sql/mod.rs`) | Phase 84 (per-backend impls) | Trait stub per spike 005; impls deferred |
| Backend-core migration | pmcp-run sibling repo (external) | Toolkit (smoke test mirrors construction surface) | D-03: verified via in-toolkit test, not cross-repo CI |
| Workspace integration | Root `Cargo.toml` + CLAUDE.md publish order | — | D-08: insertion-point edit |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pmcp` | 2.8.1+ (workspace) | Provides `ToolInfo`, `ToolAnnotations`, `ToolHandler`, `PromptHandler`, `ResourceHandler`, `AuthProvider`, `ServerBuilder::{tool_arc, prompt_arc, resources_arc, auth_provider_arc}` | The toolkit IS a pmcp consumer; D-05 workspace-version trick uses `pmcp = { version = "2.x", path = "../.." }` [VERIFIED: root Cargo.toml line 2-3] |
| `pmcp-code-mode` | 0.5.1+ (workspace, feature-gated `code-mode`) | Provides `CodeExecutor`, `CodeModeConfig`, `TokenSecret`, `HmacTokenGenerator`, `NoopPolicyEvaluator`, `ApprovalToken`, `ValidationPipeline` | D-06 + D-16 re-export only [VERIFIED: `crates/pmcp-code-mode/src/lib.rs:118-186`] |
| `serde` | 1.0 + `derive` feature | Config struct (de)serialization | Workspace convention [VERIFIED: workspace dep pattern] |
| `serde_json` | 1.0 + `raw_value, preserve_order` | JSON Schema construction for ToolInfo input schemas | Workspace convention; PMCP convention prefers `raw_value` for protocol types [VERIFIED: root Cargo.toml:51] |
| `toml` | 1.0 | `ServerConfig::from_toml(&str)` parsing | Already in pmcp's root deps line 63 and mcp-server-common's stack [VERIFIED: root Cargo.toml:63] |
| `async-trait` | 0.1 | `SqlConnector` trait, `pmcp_code_mode::async_trait` re-export | Standard workspace pattern [VERIFIED: workspace] |
| `thiserror` | 2.0 | Toolkit error type (`ToolkitError` enum) | PMCP convention — every crate uses thiserror [VERIFIED: root Cargo.toml:55] |
| `indexmap` | 2.10 + `serde` feature | Deterministic iteration for `[[tools]]` + `[[prompts]]` + `[[resources]]` lists | Same pattern used in `src/server/skills.rs` [VERIFIED: `src/server/skills.rs:41` + Cargo.toml:68] |

### Supporting (feature-gated)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `rusqlite` | 0.39 + `bundled` feature | SQLite connector for dev/CI per D-14 | `sqlite` feature (test/dev only; Phase 84 uses for fixtures) |
| `aws-sdk-secretsmanager` | latest stable + `default-https-client` (NOT rustls — RUSTSEC-2026-0098/9/0104) | `SecretsProvider::SecretsManagerSecrets` impl | `aws` feature |
| `aws-sdk-ssm` | latest stable + `default-https-client` | `SecretsProvider::SsmSecrets` impl | `aws` feature |
| `aws-config` | 1.x + `default-https-client` | AWS SDK setup; transitive from pmcp-code-mode `avp` feature | `aws` or `avp` features |
| `jsonschema` | 0.46 (no default features) | Optional input-validation of tool args against synthesized schemas | `input-validation` feature; mirrors pmcp's `validation` feature pattern [VERIFIED: root Cargo.toml:106] |

### Re-exported from `pmcp-code-mode` (via `toolkit::code_mode`)

| Symbol | Source | Gate |
|--------|--------|------|
| `CodeExecutor` trait | `pmcp_code_mode::CodeExecutor` | `code-mode` (default) |
| `TokenSecret`, `HmacTokenGenerator`, `TokenGenerator`, `ApprovalToken` | `pmcp_code_mode::token::*` | `code-mode` |
| `hash_code`, `canonicalize_code`, `compute_context_hash` | `pmcp_code_mode::token::*` | `code-mode` |
| `NoopPolicyEvaluator`, `PolicyEvaluator` trait | `pmcp_code_mode::policy::*` | `code-mode` |
| `CodeModeConfig`, `ValidationPipeline`, `ValidationContext` | `pmcp_code_mode::{config, validation}::*` | `code-mode` |
| `AvpPolicyEvaluator`, `AvpClient`, `AvpConfig` | `pmcp_code_mode::avp::*` | `code-mode` + `avp` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Standard `code-mode` feature default | `default = []` (opt-in) | CONTEXT.md D-12 rationale: `[code_mode]` is core to v2.2's value prop; defaulting on makes Shape C ≤15-line `main.rs` reachable. Opt-out path still exists for curated-only servers. |
| Single monolithic `tools::synthesize_from_config` | Per-backend synthesizer (sql/graphql/openapi) | Spike 003 finding: synthesis from `[[tools]]` config IS shared (it just produces ToolInfo + Arc<dyn ToolHandler>); only the connector inside the handler diverges. Keep one synthesizer; backend-specific handlers compose underneath. |
| `pmcp-server-toolkit` direct dep on `pmcp-code-mode` (no feature gate) | Feature-gated `code-mode` | D-06: feature gate keeps the door open for pure-curated-tools servers that don't need code-mode (smaller dep graph for those users). |
| Bundle `aws` + `avp` as one feature | Split features | Different users want different subsets — some want AWS Secrets Manager without AVP, some want AVP without S3. Match `pmcp-code-mode`'s split: `avp` already a separate feature there. |

**Installation:**
```bash
# Add to crates/pmcp-server-toolkit/Cargo.toml
[dependencies]
pmcp = { version = "2.8.1", path = "../..", default-features = false }
pmcp-code-mode = { version = "0.5.1", path = "../pmcp-code-mode", optional = true }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "1"
async-trait = "0.1"
thiserror = "2"
indexmap = { version = "2.10", features = ["serde"] }
tracing = "0.1"

[features]
default = ["code-mode"]
code-mode = ["dep:pmcp-code-mode"]
aws = ["dep:aws-config", "dep:aws-sdk-secretsmanager", "dep:aws-sdk-ssm"]
avp = ["code-mode", "pmcp-code-mode/avp"]
input-validation = ["dep:jsonschema"]
sqlite = ["dep:rusqlite"]
```

**Version verification (verified 2026-05-18):**
- `hmac = "0.13.0"` [VERIFIED: cargo search]
- `secrecy = "0.10.3"` [VERIFIED: cargo search]
- `rusqlite = "0.39.0"` [VERIFIED: cargo search]
- `jsonschema = "0.46.5"` [VERIFIED: cargo search]
- `pmcp@2.8.1` (workspace root) [VERIFIED: root Cargo.toml:3]
- `pmcp-code-mode@0.5.1` (workspace) [VERIFIED: root Cargo.toml:148]
- `mcp-tester@0.7.0` (next member, slots after toolkit) [VERIFIED: `crates/mcp-tester/Cargo.toml:3`]

## Architecture Patterns

### System Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────────────┐
│                        Toolkit Consumer (downstream)                      │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │  main.rs (Shape C, ≤15 lines)                                       │  │
│  │   ┌──────────────────────────────────────────────────────────┐    │  │
│  │   │  ServerConfig::from_toml(include_str!("config.toml"))     │    │  │
│  │   │      ↓                                                     │    │  │
│  │   │  pmcp::Server::builder()                                   │    │  │
│  │   │      .tools_from_config(&config)        ← ServerBuilderExt│    │  │
│  │   │      .code_mode_from_config(&config)    ← ServerBuilderExt│    │  │
│  │   │      .resources(StaticResourceHandler::from(&config))     │    │  │
│  │   │      .prompts(StaticPromptHandler::from(&config))          │    │  │
│  │   │      .auth_provider_arc(...)            (Phase 82 method) │    │  │
│  │   │      .build()                                              │    │  │
│  │   └──────────────────────────────────────────────────────────┘    │  │
│  └────────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼  (uses)
┌──────────────────────────────────────────────────────────────────────────┐
│                   pmcp-server-toolkit (this phase)                        │
│                                                                            │
│   ┌─────────────────┐    ┌──────────────────┐   ┌──────────────────┐    │
│   │   config.rs     │    │     tools.rs     │   │  code_mode.rs    │    │
│   │  ServerConfig   │───▶│ synthesize_from_ │   │ executor_from_   │    │
│   │  (deny_unknown) │    │     config()     │   │     config()     │    │
│   │  from_toml()    │    │ ServerBuilderExt │   │ ServerBuilderExt │    │
│   └────────┬────────┘    │ .tools_from_     │   │ .code_mode_from_ │    │
│            │             │     config()     │   │     config()     │    │
│            │             └─────────┬────────┘   │ assemble_code_   │    │
│            │                       │             │  mode_prompt()  │    │
│            │                       │             └────────┬─────────┘    │
│            │                       ▼                      │              │
│            │              ┌──────────────────┐            │              │
│            │              │   ToolInfo + Arc │            │              │
│            │              │   <ToolHandler>  │            │              │
│            │              └──────────────────┘            │              │
│            │                                              ▼              │
│   ┌────────▼────────┐                          ┌─────────────────────┐  │
│   │   auth.rs       │                          │     sql/mod.rs      │  │
│   │   secrets.rs    │  (verbatim lifts         │  SqlConnector trait │  │
│   │   resources.rs  │   from                   │  Dialect enum       │  │
│   │   prompts.rs    │   mcp-server-common)     │  (stubs; impls P84) │  │
│   └─────────────────┘                          └─────────────────────┘  │
│                                                                            │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                ┌───────────────────┼───────────────────┐
                ▼                                       ▼
   ┌────────────────────────┐              ┌──────────────────────────┐
   │       pmcp             │              │     pmcp-code-mode       │
   │   (workspace dep)      │              │   (feature-gated dep)    │
   │                        │              │                          │
   │  ToolInfo              │              │  CodeExecutor trait      │
   │  ToolAnnotations       │              │  CodeModeConfig          │
   │  ToolHandler           │              │  TokenSecret             │
   │  PromptHandler         │              │  HmacTokenGenerator      │
   │  ResourceHandler       │              │  NoopPolicyEvaluator     │
   │  AuthProvider          │              │  ValidationPipeline      │
   │  ServerBuilder         │              │  (+ AVP behind avp feat) │
   │  .tool_arc / prompt_   │              │                          │
   │   arc / resources_arc  │              │                          │
   └────────────────────────┘              └──────────────────────────┘

Component responsibilities mapped to files:
- ServerConfig parse/validate         → config.rs (net-new)
- AuthProvider impls (Static, Bearer) → auth.rs (lift from mcp-server-common)
- SecretsProvider impls (Env, AWS)    → secrets.rs (lift, ~701 LoC)
- StaticResourceHandler                → resources.rs (lift, ~333 LoC)
- StaticPromptHandler                  → prompts.rs (lift, ~285 LoC)
- HMAC token helpers                   → code_mode.rs (re-export only)
- [[tools]]→ToolInfo synthesizer       → tools.rs (NET-NEW)
- [code_mode]→CodeExecutor bridge      → code_mode.rs (NET-NEW)
- Prompt assembler + SqlConnector stub → sql/mod.rs + code_mode.rs (NET-NEW)
- ServerBuilderExt trait               → lib.rs (NET-NEW; bundles .tools_from_config + .code_mode_from_config)
- Smoke test against backend cores     → tests/backend_core_smoke.rs (D-03)
- Reference config parse test          → tests/reference_configs.rs (D-17)
```

### Recommended Project Structure

```
crates/pmcp-server-toolkit/
├── Cargo.toml
├── README.md                     # CRATE-README, dual-published to docs.rs
├── src/
│   ├── lib.rs                    # Module declarations + crate-root re-exports + ServerBuilderExt trait (D-15)
│   ├── auth.rs                   # AuthProvider impls (lift from mcp-server-common)
│   ├── secrets.rs                # SecretsProvider trait + impls (lift)
│   ├── resources.rs              # StaticResourceHandler (lift)
│   ├── prompts.rs                # StaticPromptHandler (lift)
│   ├── config.rs                 # ServerConfig + ServerSection + MetadataSection +
│   │                             #   DatabaseSection + [[database.tables]] +
│   │                             #   [[tools]] + [[tools.parameters]] + [tools.annotations] +
│   │                             #   [code_mode] + [code_mode.limits] + [[prompts]] + [[resources]]
│   │                             #   ALL with #[serde(deny_unknown_fields)] per D-13
│   ├── tools.rs                  # synthesize_from_config() + SynthesizedToolHandler internal type
│   ├── code_mode.rs              # executor_from_config() + assemble_code_mode_prompt()
│   │                             #   + re-exports of pmcp-code-mode types (gated)
│   ├── sql/
│   │   └── mod.rs                # SqlConnector trait stub + Dialect enum stub (per spike 005)
│   ├── error.rs                  # ToolkitError thiserror enum (private detail; Result<T> alias)
│   └── builder_ext.rs            # ServerBuilderExt trait + impl on pmcp::ServerBuilder
├── tests/
│   ├── backend_core_smoke.rs     # D-03: replays mcp-sql/graphql/openapi-core construction
│   ├── reference_configs.rs      # D-17: parses open-images/imdb/msr-vtt config.tomls
│   ├── tool_synthesis_props.rs   # proptest: [[tools]]→ToolInfo round-trip
│   └── code_mode_wiring.rs       # integration: [code_mode]→CodeExecutor policy enforcement
├── examples/
│   └── e01_toolkit_minimal.rs    # CLAUDE.md ALWAYS: example demonstrating real usage
├── fuzz/                         # Toolkit's own fuzz target (separate from cargo-pmcp's)
│   ├── Cargo.toml
│   └── fuzz_targets/
│       └── pmcp_server_toolkit_config_parser.rs
└── fixtures/                     # Reference config.toml copies for tests + the shim diff artifact
    ├── open-images-config.toml   # Copied snapshot — symlinks don't work cross-platform on Win
    ├── imdb-config.toml
    └── msr-vtt-config.toml
```

### Pattern 1: Verbatim Lift with Trait Reuse

**What:** Copy `mcp-server-common/src/{auth,secrets,resources,prompts}.rs` into the toolkit, adjusting only:
- Crate path: `mcp_server_common::*` → `pmcp_server_toolkit::*`
- Cargo features: drop `ddb`, `dynamo-config`, `openapi-code-mode`, `js-runtime`, `mcp-code-mode` per D-14
- pmcp dependency declaration: workspace-version-trick per D-05

**When to use:** auth.rs (~600 LoC), secrets.rs (701 LoC), resources.rs (333 LoC), prompts.rs (285 LoC) — total ~1900 LoC of lift, zero refactoring.

**Example skeleton** (auth.rs StaticAuthProvider — verify exact shape against mcp-server-common):
```rust
// crates/pmcp-server-toolkit/src/auth.rs
use async_trait::async_trait;
use pmcp::{AuthContext, AuthProvider, Result as PmcpResult};

/// Static bearer-token auth, suitable for dev and tests.
/// Promoted verbatim from `mcp-server-common::auth::StaticAuthProvider`.
pub struct StaticAuthProvider {
    expected_token: String,
}

#[async_trait]
impl AuthProvider for StaticAuthProvider {
    async fn validate_request(&self, header: Option<&str>) -> PmcpResult<Option<AuthContext>> {
        // ... identical body to mcp-server-common
    }
}
```

### Pattern 2: ToolInfo Synthesizer (Net-New)

**What:** Turn a `[[tools]]` config entry + its `[[tools.parameters]]` + `[tools.annotations]` into a complete pmcp `ToolInfo` + a synthesized `Arc<dyn ToolHandler>`.

**When to use:** TKIT-07 core — runs once at server-build time. The handler captures the SQL/operation declaration and at call time binds args → calls connector.

**Example skeleton** [CITED: spike 004 source + `src/types/tools.rs:176-264`]:
```rust
// crates/pmcp-server-toolkit/src/tools.rs
use pmcp::types::{ToolAnnotations, ToolInfo};
use pmcp::server::ToolHandler;
use serde_json::{json, Map, Value};
use std::sync::Arc;

use crate::config::{ServerConfig, ToolDecl, ParamDecl};
use crate::error::Result;

/// Synthesize one ToolInfo + handler per `[[tools]]` config entry.
/// Returns owned data the caller registers via `pmcp::ServerBuilder::tool_arc`.
pub fn synthesize_from_config(
    config: &ServerConfig,
) -> Result<Vec<(String, ToolInfo, Arc<dyn ToolHandler>)>> {
    let mut out = Vec::with_capacity(config.tools.len());
    for decl in &config.tools {
        let schema = build_input_schema(&decl.parameters);
        let annotations = build_annotations(&decl.annotations);
        let info = ToolInfo::with_annotations(
            decl.name.clone(),
            Some(decl.description.clone()),
            schema,
            annotations,
        );
        let handler: Arc<dyn ToolHandler> =
            Arc::new(SynthesizedToolHandler::new(decl.clone()));
        out.push((decl.name.clone(), info, handler));
    }
    Ok(out)
}

fn build_input_schema(params: &[ParamDecl]) -> Value {
    let mut props = Map::new();
    let mut required = Vec::new();
    for p in params {
        let mut prop = json!({
            "type": p.ty.json_schema_type(),
            "description": p.description,
        });
        if let Some(min) = p.min { prop["minimum"] = json!(min); }
        if let Some(max) = p.max { prop["maximum"] = json!(max); }
        if let Some(max_len) = p.max_length { prop["maxLength"] = json!(max_len); }
        if let Some(default) = &p.default { prop["default"] = default.clone(); }
        props.insert(p.name.clone(), prop);
        if p.required { required.push(Value::String(p.name.clone())); }
    }
    json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false,
    })
}

fn build_annotations(decl: &crate::config::AnnotationsDecl) -> ToolAnnotations {
    ToolAnnotations::new()
        .with_read_only(decl.read_only_hint.unwrap_or(true))
        // ... and for each other annotation field
}
```

### Pattern 3: Builder Extension Trait

**What:** Add `.tools_from_config(&config)` and `.code_mode_from_config(&config)` to `pmcp::ServerBuilder` via an extension trait that toolkit consumers `use`.

**When to use:** Decisions D-10 + D-11 require both a low-level `fn` AND a builder extension. The trait composes both: low-level for power users, builder method for Shape C ≤15-line `main.rs`.

**Example skeleton:**
```rust
// crates/pmcp-server-toolkit/src/builder_ext.rs
use pmcp::ServerBuilder;
use crate::config::ServerConfig;
use crate::tools::synthesize_from_config;

pub trait ServerBuilderExt {
    fn tools_from_config(self, config: &ServerConfig) -> Self;
    fn code_mode_from_config(self, config: &ServerConfig) -> Self;
}

impl ServerBuilderExt for ServerBuilder {
    fn tools_from_config(mut self, config: &ServerConfig) -> Self {
        let synthesized = synthesize_from_config(config)
            .expect("synthesize_from_config: ServerConfig is invariant-checked at parse time");
        for (name, _info, handler) in synthesized {
            // Phase 82's tool_arc — registers handler; ToolInfo metadata comes from handler.metadata()
            self = self.tool_arc(name, handler);
        }
        self
    }

    fn code_mode_from_config(mut self, config: &ServerConfig) -> Self {
        #[cfg(feature = "code-mode")]
        {
            // Build CodeExecutor + register validate_code/execute_code tools
            // See code_mode.rs::executor_from_config
        }
        self
    }
}
```

### Anti-Patterns to Avoid

- **Don't reintroduce `mcp-server-common`'s `ddb` / `dynamo-config` / `openapi-code-mode` / `js-runtime` / `mcp-code-mode` features.** D-14 explicitly drops them — they are pmcp-run-specific or Phase 3 OpenAPI territory.
- **Don't redefine `AuthProvider` / `ResourceHandler` / `PromptHandler` traits.** Use `pmcp::AuthProvider` at `src/server/auth/traits.rs:450` and `pmcp::{ResourceHandler, PromptHandler}` at `src/server/mod.rs:{256,238}`. Toolkit provides *impls* of these traits, not new traits.
- **Don't redefine HMAC token machinery.** D-16: re-export from `pmcp-code-mode`. The TokenSecret type at `crates/pmcp-code-mode/src/token.rs:38` is already secrecy/zeroize-backed with intentionally-denied Debug/Clone/Serialize per CMSUP-02.
- **Don't put `translate_placeholders` on the SqlConnector trait.** Spike 005 explicitly warns against this (`schema-server-sql-dialects.md` §"What to Avoid"): free helpers prevent per-backend overrides that introduce subtle drift.
- **Don't loosen `#[serde(deny_unknown_fields)]` to satisfy REF-01 superset.** D-13: REF-01 is enforced by ADDING fields, not by tolerating typos.
- **Don't use struct-literal syntax for `#[non_exhaustive]` types.** From spike findings: use `CallToolResult::new(content)`, `ToolInfo::with_annotations(...)`, `PromptMessage::user(content)` — not literal struct construction (the types are marked `#[non_exhaustive]` to permit additive evolution).
- **Don't bake AVP/Cedar specifically into the toolkit.** D-14 + spike findings: ship `NoopPolicyEvaluator` default; AVP via `avp` feature only (re-export from pmcp-code-mode).
- **Don't lift `pmcp-run/built-in/shared/`'s test files verbatim.** D-17 calls for new test files specifically targeting toolkit invariants (config round-trip, ToolInfo property, all-three-reference-configs integration). Old tests had different focus (auth flow, secrets retrieval).
- **Don't write a 20-line delegating wrapper around `ServerCoreBuilder::tool_arc`.** Phase 82 lifted `tool_arc` / `prompt_arc` to the public `ServerBuilder` specifically to eliminate this paper-cut.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Bearer-token auth scaffolding | Custom `AuthProvider` impl | Lift `StaticAuthProvider` from `mcp-server-common/src/auth.rs` | Tested production-grade; mcp-server-common's lib has been live across three pmcp-run backends |
| HMAC sign/verify | New HMAC code | `pmcp_code_mode::HmacTokenGenerator` + `TokenSecret` | secrecy/zeroize-backed per CMSUP-02; intentional non-Clone non-Debug discipline already enforced |
| Code canonicalization for token binding | Custom hasher | `pmcp_code_mode::canonicalize_code` + `hash_code` | Already proven across SQL/GraphQL/OpenAPI in pmcp-code-mode |
| Policy evaluation | Custom decision engine | `pmcp_code_mode::NoopPolicyEvaluator` (default) + AVP behind `avp` feature | Pluggable via `PolicyEvaluator` trait; AWS-specific lives in pmcp-code-mode/avp.rs |
| Config TOML parsing primitives | Manual parser | `toml = "1.0"` + serde derive + `deny_unknown_fields` | Workspace convention; same crate parses `~/.pmcp/config.toml` in cargo-pmcp |
| JSON Schema construction for tool input | String-templated JSON | `serde_json::json!{...}` macro | Standard pattern in pmcp `src/server/typed_tool.rs` |
| Secrecy newtype for HMAC keys | Custom wrapper | `secrecy::SecretBox` | pmcp-code-mode already uses this; consistency wins |
| Resource MIME-typed serving | Inline MIME logic | `Content::resource_with_text` (used by `src/server/skills.rs:14-16`) | Wire shape stable; MIME types survive wire round-trip |
| Pre-commit quality gates | Custom hook | `make quality-gate` | Mandatory per CLAUDE.md |

**Key insight:** The toolkit is ~80% a lift and ~20% net-new. The lift items have proven production callers (three pmcp-run backends); reproducing them invites drift. Net-new code is small (synthesizer ~200 LoC, code-mode wiring ~150 LoC, prompt assembler ~80 LoC, builder extension ~60 LoC). Total estimated toolkit LoC: 2500–3000 (vs spike-estimated 2200).

## Common Pitfalls

### Pitfall 1: `#[serde(deny_unknown_fields)]` + REF-01 superset tension

**What goes wrong:** Reference servers' config.tomls have fields not currently in `ServerConfig` → strict parsing fails → REF-01 superset broken → P83 SC-2 fails.

**Why it happens:** D-13 mandates `deny_unknown_fields` on all sections. REF-01 mandates the toolkit be a *superset*. The only path through is to enumerate every field that appears in any of the three reference configs (open-images, imdb, msr-vtt) BEFORE adding `deny_unknown_fields`.

**How to avoid:** Plan a step that diffs the union of all three reference configs against the proposed `ServerConfig` struct fields, adding any missing fields. The integration test (D-17 + `tests/reference_configs.rs`) closes the loop.

**Warning signs:** First run of `cargo test --test reference_configs` fails with `unknown field "foo"`. Fix: add the field to ServerConfig (with `#[serde(default)]` if optional).

### Pitfall 2: pmcp-code-mode `code-mode` feature naming collision

**What goes wrong:** Toolkit's `code-mode` feature toggles `pmcp-code-mode` dep; pmcp-code-mode itself has features like `openapi-code-mode`, `sql-code-mode`, `mcp-code-mode`. Confusion about which one drives the toolkit.

**Why it happens:** The toolkit's `code-mode` feature is a single switch for "include pmcp-code-mode at all"; the pmcp-code-mode features are about which validators ship. D-14 explicitly excludes `openapi-code-mode`, `js-runtime`, `mcp-code-mode` from the toolkit's pmcp-code-mode dep.

**How to avoid:** Toolkit's dep declaration:
```toml
pmcp-code-mode = { version = "0.5.1", path = "../pmcp-code-mode", default-features = false, optional = true }
```
Note `default-features = false`. The toolkit's `avp` feature explicitly enables `pmcp-code-mode/avp`. SQL validation (`sql-code-mode`) is NOT a toolkit feature in 0.1.0 (per D-14, SQLite-the-connector ships separately; the SQL *validator* in pmcp-code-mode is loaded by SQL backend cores in pmcp-run).

**Warning signs:** Either the toolkit pulls in swc_ecma_parser or sqlparser unnecessarily (feature leak), or `[code_mode]` validation silently doesn't validate SQL (feature missing).

### Pitfall 3: Phase 82 method calls without verifying the public re-export

**What goes wrong:** Plan uses `pmcp::ServerBuilder::tool_arc` but pmcp 2.8.x doesn't actually re-export it at the top-level path.

**Why it happens:** Phase 82 verification confirms `pub fn tool_arc` exists on `pmcp::ServerBuilder` (re-exported as `pmcp::ServerBuilder` from `pmcp::server::ServerCoreBuilder`); the test `tests/in_process_handler_pattern.rs` proves this.

**How to avoid:** Verify in the plan check by reading `src/server/builder.rs:203` (tool_arc), `:254` (prompt_arc), `:294` (resources_arc), `:420-445` (sampling_arc + auth_provider_arc) [VERIFIED 2026-05-18 via grep]. Plus confirm `pmcp::ServerBuilder` is at `pmcp::ServerBuilder` (it is — re-exported in `src/lib.rs`).

**Warning signs:** Compile error `no method named tool_arc`. Likely fix: import path mismatch — should be `pmcp::ServerBuilder` not `pmcp::server::Server::builder()`.

### Pitfall 4: Workspace-version dep "circular path" gotcha

**What goes wrong:** Toolkit declares `pmcp = { version = "2.x", path = "../.." }`, but pmcp's root Cargo.toml lists `pmcp-server-toolkit` in `[workspace.members]` → cargo's workspace resolver sees a path dep that points back to the root and may warn about path/version mismatch.

**Why it happens:** D-05 uses the workspace-version trick: locally, cargo uses the path; on `cargo publish` it uses the version. This is the same pattern `crates/mcp-tester/Cargo.toml:21` uses (`pmcp = { version = "2.8.1", path = "../../", ... }`).

**How to avoid:** Verify the pattern by reading `crates/mcp-tester/Cargo.toml:21` [VERIFIED 2026-05-18]. Use identical syntax: `path = "../.."` (or `"../../"` — both work; pick one and stay consistent).

**Warning signs:** `cargo publish --dry-run` complains about path dep without version, OR local `cargo check` complains about version mismatch.

### Pitfall 5: Cognitive complexity ceiling on synthesizer

**What goes wrong:** `synthesize_from_config` ends up doing param → schema, annotation → ToolAnnotations, handler construction, and aliasing in one fn → exceeds CLAUDE.md / Phase 75 cog 25 ceiling → CI gate blocks PR.

**Why it happens:** It's tempting to handle all param types (string/integer/boolean/array) and all annotation fields in one big match.

**How to avoid:** Decompose `synthesize_from_config` into ≤3 helper fns: `build_input_schema(&[ParamDecl])`, `build_annotations(&AnnotationsDecl)`, and the loop body. Each ≤25 cog. Annotated `#[allow(clippy::cognitive_complexity)]` with `// Why:` is permitted only if irreducible (per Phase 75 D-03); P83's synthesizer is NOT irreducible.

**Warning signs:** PMAT in CI reports `complexity violations` for `tools.rs`.

### Pitfall 6: Crates.io 10MB publish limit (recurring trap)

**What goes wrong:** Toolkit accidentally bundles fixtures or .planning artifacts; `cargo publish` fails or v0.1.0 ships with bloat.

**Why it happens:** Memory `feedback_v2_cleanup.md` shows pmcp 2.2.0 release blew the 10MB limit twice — once via `.pmat/context.db` (~21MB), once via `.planning/` (6.3MB).

**How to avoid:** Toolkit's `Cargo.toml` must include `exclude = [".planning/", ".pmat/", "fixtures/"]` (or equivalent `include = [...]` whitelist of `src/`, `tests/`, `README.md`, `Cargo.toml`). Verify with `cargo package --list` before publish.

**Warning signs:** `cargo package --list` output > 50 files OR includes anything outside `src/`, `tests/`, `examples/`, `README.md`, `Cargo.toml`, `LICENSE`.

## Runtime State Inventory

> This phase creates a new crate; no rename/refactor/migration. Section omitted.

## Code Examples

### Common Operation 1: ServerConfig parse + tools_from_config wiring (Shape C target)

```rust
// Source: spike 004 (validated 12-line surface) + Phase 82 BLDR-01..04 verification
use pmcp::Server;
use pmcp_server_toolkit::{
    config::ServerConfig,
    ServerBuilderExt,
    StaticResourceHandler,
    StaticPromptHandler,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig::from_toml(include_str!("config.toml"))?;
    let server = Server::builder()
        .name(&config.server.name)
        .version(&config.server.version)
        .tools_from_config(&config)            // D-10: synthesizes all [[tools]]
        .code_mode_from_config(&config)        // D-11: wires [code_mode] → CodeExecutor
        .resources(StaticResourceHandler::from(&config))
        .prompts(StaticPromptHandler::from(&config))
        .build()?;
    server.run_stdio().await?;
    Ok(())
}
```

### Common Operation 2: Synthesize input schema from `[[tools.parameters]]`

```rust
// Source: schema-server-architecture.md §"Toolkit's per-tool handler synthesis" +
//          pmcp::types::tools::ToolInfo at src/types/tools.rs:209
use pmcp::types::{ToolAnnotations, ToolInfo};
use serde_json::json;

let schema = json!({
    "type": "object",
    "properties": {
        "max_results": {
            "type": "integer",
            "description": "Maximum rows to return",
            "minimum": 1,
            "maximum": 1000,
            "default": 100,
        }
    },
    "required": [],
    "additionalProperties": false,
});

let annotations = ToolAnnotations::new()
    .with_read_only(true)
    .with_destructive(false)
    .with_idempotent(true)
    .with_open_world(false);

let info = ToolInfo::with_annotations(
    "search_images",
    Some("Search image dataset".to_string()),
    schema,
    annotations,
);
```

### Common Operation 3: Code-mode prompt assembler (TKIT-10)

```rust
// Source: spike 005 `build_code_mode_prompt` + D-12 assembly fn
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::sql::SqlConnector;

pub fn assemble_code_mode_prompt(
    connector: &dyn SqlConnector,
    config: &ServerConfig,
) -> String {
    let schema_text = connector.schema_text_sync();  // Phase 84 supplies real impl
    let dialect_guidance = connector.dialect().placeholder_guidance();
    let curated_descriptions = config.database.tables
        .iter()
        .map(|t| format!("- `{}`: {}", t.name, t.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "# Code Mode — {dialect}\n\n\
        {dialect_guidance}\n\n\
        ## Schema\n\n{schema_text}\n\n\
        ## Curated Tables\n\n{curated_descriptions}\n",
        dialect = connector.dialect().name(),
    )
}
```

### Common Operation 4: HMAC token re-export pass-through (TKIT-06)

```rust
// Source: D-16 + crates/pmcp-code-mode/src/lib.rs:160-163
// In crates/pmcp-server-toolkit/src/code_mode.rs:

#[cfg(feature = "code-mode")]
pub use pmcp_code_mode::{
    // HMAC token machinery (TKIT-06)
    ApprovalToken, HmacTokenGenerator, TokenGenerator, TokenSecret,
    canonicalize_code, compute_context_hash, hash_code,
    // Execution surface
    CodeExecutor,
    // Policy
    NoopPolicyEvaluator, PolicyEvaluator, AuthorizationDecision,
    // Pipeline
    ValidationContext, ValidationPipeline,
    // Config
    CodeModeConfig,
};

#[cfg(all(feature = "code-mode", feature = "avp"))]
pub use pmcp_code_mode::{AvpClient, AvpConfig, AvpPolicyEvaluator};
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `Vec<u8>` token secrets | `secrecy::SecretBox<[u8]>` newtype (`TokenSecret`) | Phase 67.1 (CMSUP-02) | Memory zeroing on drop; intentional non-Clone non-Debug |
| `#[tool]` / `#[tool_router]` proc macros | `#[mcp_tool]` / `#[mcp_server]` (will be PARITY-MACRO-01 → MACR-01..03) | Phase 71 (rustdoc fallback) | Toolkit doesn't use macros — uses synthesizer fn |
| `ServerCoreBuilder::tool_arc` (crate-internal) | `pmcp::ServerBuilder::tool_arc` (public) | Phase 82 (BLDR-01..04) | Toolkit's builder extension can use `tool_arc` without a 20-line wrapper |
| Single `mcp-server-common` shared in pmcp-run path-deps | Public `pmcp-server-toolkit` on crates.io | This phase (P83) | Independent release cadence; external developers can consume |
| `mcp-server-common` 9 features (incl. `ddb`, `openapi-code-mode`) | Toolkit slim 5 features (`code-mode`, `aws`, `avp`, `input-validation`, `sqlite`) | This phase (D-14) | Drops pmcp-run-specific (ddb, dynamo-config) and Phase-3 (openapi-code-mode, js-runtime, mcp-code-mode) |

**Deprecated/outdated:**
- `pmcp-run/built-in/shared/mcp-server-common/` direct path-deps from the three backend cores — will be re-export shim after P83's operator handoff (D-01, D-02).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `mcp-server-common` lifts cleanly with only crate-path + feature changes (no per-file refactoring) | Pattern 1 | LOW — the proto-SDK is already extracted at pmcp-run/built-in/shared/; spike 003 verified the shape. Worst case: a per-file delta pass adds ~1 day to a plan |
| A2 | `pmcp_code_mode::CodeModeConfig` (at `crates/pmcp-code-mode/src/config.rs:95`) directly accepts CONTEXT.md's `[code_mode]` fields with aliases for `allow_writes` / `allow_deletes` / `allow_ddl` / `blocked_tables` etc | Code-Mode Wiring | LOW — verified lines 189–236 already use `alias = "allow_writes"` etc. Worst case: add `[code_mode.limits]` (`max_tables_per_query`, `max_join_depth`, `max_subquery_depth`) fields to a new sub-struct since current CodeModeConfig uses flat names `max_depth`, `max_field_count`, `max_cost` |
| A3 | `pmcp::ServerBuilder` accepts the workspace-version dep pattern (i.e., `pmcp = { version = "2.x", path = "../.." }`) without dependency-cycle warnings | Pitfall 4 | LOW — verified `crates/mcp-tester/Cargo.toml:21` already uses this exact pattern with no CI complaints |
| A4 | All three reference config.tomls (open-images, imdb, msr-vtt) fit the proposed `ServerConfig` shape without additive fields beyond what CONTEXT.md enumerates | Pitfall 1 | MEDIUM — they live in pmcp-run repo, not visible here. The lift's first integration test will surface any missing fields. Worst case: ~1 day to enumerate + add fields |
| A5 | Crates.io accepts `pmcp-server-toolkit` as a name (not squatted) | Workspace + Cargo.toml | LOW — `cargo search pmcp-server-toolkit` returns no exact match. Worst case: pick alternate name like `pmcp-toolkit-core` |

**If this table has items rated MEDIUM or higher:** Treat them as discussion points during plan-check. A4 specifically: the planner should ensure one of the first plans is "fetch/snapshot all three reference config.tomls into `crates/pmcp-server-toolkit/fixtures/` and verify-parse before declaring config.rs done."

## Open Questions

1. **Should we ship a contract YAML for the toolkit's public API?**
   - What we know: CLAUDE.md §"Contract-First Development" mandates contracts for new features. Two existing contracts live at `contracts/{binding.yaml, mcp-protocol-sdk-v1.yaml}`.
   - What's unclear: New file `contracts/toolkit-v1.yaml` vs extending `binding.yaml` with toolkit rows.
   - Recommendation: Extend `binding.yaml` (smaller diff, planner can map TKIT-02..07 to rows in 1 plan). Decide during discuss-phase.

2. **`#[derive(CodeMode)]` macro usage in toolkit?**
   - What we know: `crates/pmcp-code-mode-derive/` provides `#[derive(CodeMode)]` per CMSUP-04. It emits `register_code_mode_tools(builder)` for a struct.
   - What's unclear: Does the toolkit's `code_mode_from_config` use the derive macro internally, or does it manually register `validate_code` + `execute_code` tools?
   - Recommendation: Manual registration. The derive macro is for application code (one struct → its handlers); the toolkit's job is to register code-mode tools generically for any config. Manual registration in `code_mode.rs::register_code_mode_tools()` is one ~40-LoC fn.

3. **WASM compatibility**
   - What we know: `pmcp` supports `wasm32-unknown-unknown` via `wasm` feature. `pmcp-code-mode` uses tokio (sync feature only) but includes `swc_ecma_parser` (openapi-code-mode) and `sqlparser` (sql-code-mode) — both probably block WASM.
   - What's unclear: Is the toolkit expected to compile to WASM?
   - Recommendation: NO. Spike findings + CONTEXT.md don't mention WASM. The toolkit targets Lambda + local; WASM is a separate concern. Document explicitly in the README that `pmcp-server-toolkit` does not support `wasm32-*` targets in 0.1.0. The base `pmcp` crate's WASM build remains unaffected.

4. **Verbatim lift mechanics: copy files, or vendor as submodule?**
   - What we know: D-01..D-04 describe an asymmetric lift (toolkit lands in rust-mcp-sdk; pmcp-run gets the shim later).
   - What's unclear: Does the lift literally `cp pmcp-run/built-in/shared/mcp-server-common/src/*.rs crates/pmcp-server-toolkit/src/`?
   - Recommendation: Yes — direct file copy with per-file commit attribution noting the source. Vendoring as submodule would tie release cadences together, defeating the whole point of D-08. Add a header comment per file: `// Originated from pmcp-run/built-in/shared/mcp-server-common/src/{file}.rs (https://github.com/guyernest/pmcp-run)`. Same pattern as `crates/pmcp-code-mode/src/lib.rs:1`.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust stable toolchain | All | ✓ | 1.95.0 (matches CI dtolnay/rust-toolchain@stable) | — |
| `cargo` (workspace) | All | ✓ | 1.95.0 | — |
| `cargo-fuzz` | Fuzz target (`pmcp_server_toolkit_config_parser`) | ✓ | installed at `/Users/guy/.cargo/bin/cargo-fuzz` | `cargo +nightly fuzz` if installation breaks |
| `make` | Quality gate | ✓ (assumed standard macOS/Linux toolchain) | — | run individual `cargo` cmds (less coverage than `make quality-gate`) |
| PMAT 3.15.0 | CI quality-gate (Phase 75) | runs in CI | 3.15.0 | — (CI-only, not blocking local dev per CLAUDE.md §"Quality Gate Enforcement") |
| Three reference config.tomls (open-images, imdb, msr-vtt) | Reference parse test (D-17) | requires manual copy from pmcp-run sibling repo | n/a | If sibling repo not accessible, planner should request operator-supplied snapshots |
| `pmcp-run` sibling repo | Shim diff artifact target | external, not in CI | n/a | D-03 + D-04 explicitly remove the dependency: shim is delivered as an artifact, not as cross-repo PR |

**Missing dependencies with no fallback:** None at toolkit build time. Reference configs (medium-risk dependency) have a recommended fallback (operator-supplied snapshots).

**Missing dependencies with fallback:** Reference configs (operator snapshots).

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` (built-in) + `proptest = "1.7"` + `quickcheck = "1.0"` + `cargo-fuzz` |
| Config file | `crates/pmcp-server-toolkit/Cargo.toml` `[dev-dependencies]` (proptest); `crates/pmcp-server-toolkit/fuzz/Cargo.toml` (fuzz) |
| Quick run command | `cargo test -p pmcp-server-toolkit --all-features` |
| Full suite command | `make test-unit && make test-property && make test-doc && make test-examples && make test-integration && make test-fuzz` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TKIT-01 | Crate compiles, all features + no features | unit | `cargo build -p pmcp-server-toolkit --all-features && cargo build -p pmcp-server-toolkit --no-default-features` | ❌ Wave 0 |
| TKIT-02 | `AuthProvider` impl works against pmcp trait | unit | `cargo test -p pmcp-server-toolkit auth::tests` | ❌ Wave 0 (lifted from mcp-server-common) |
| TKIT-03 | `SecretsProvider` impls retrieve secrets | unit | `cargo test -p pmcp-server-toolkit secrets::tests` | ❌ Wave 0 |
| TKIT-04 | `StaticResourceHandler` serves config-defined resources | unit | `cargo test -p pmcp-server-toolkit resources::tests` | ❌ Wave 0 |
| TKIT-05 | `StaticPromptHandler` serves config-defined prompts | unit | `cargo test -p pmcp-server-toolkit prompts::tests` | ❌ Wave 0 |
| TKIT-06 | HMAC sign+verify round-trips, code-hash binding | integration | `cargo test -p pmcp-server-toolkit --test code_mode_wiring hmac_*` | ❌ Wave 0 |
| TKIT-07 | `synthesize_from_config` produces ToolInfo with non-empty schema for every `[[tools]]` entry | property | `cargo test -p pmcp-server-toolkit --test tool_synthesis_props` | ❌ Wave 0 |
| TKIT-07 | All three reference config.tomls parse and produce ToolInfo vectors | integration | `cargo test -p pmcp-server-toolkit --test reference_configs` | ❌ Wave 0 |
| TKIT-08 | Toolkit covers mcp-sql-server-core construction surface | integration | `cargo test -p pmcp-server-toolkit --test backend_core_smoke` | ❌ Wave 0 (D-03 anchor) |
| TKIT-09 | `[code_mode]` parses + builds CodeExecutor + register_code_mode_tools wires validate_code/execute_code | integration | `cargo test -p pmcp-server-toolkit --test code_mode_wiring code_mode_*` | ❌ Wave 0 |
| TKIT-10 | `assemble_code_mode_prompt` includes schema_text + `[[database.tables]]` descriptions for canned MockSqlConnector | unit | `cargo test -p pmcp-server-toolkit code_mode::tests::assemble_*` | ❌ Wave 0 |
| TEST-02 | Property: placeholder translation invariants (CONN-03 trait stubs only; full impls in P84) | property | `cargo test -p pmcp-server-toolkit sql::tests::translate_*` | ❌ Wave 0 |
| TEST-02 | Property: `[[tools]]` entry → valid ToolInfo round-trip | property | `cargo test -p pmcp-server-toolkit --test tool_synthesis_props` | ❌ Wave 0 |
| TEST-03 | Every public type has a passing doctest | doctest | `cargo test --doc -p pmcp-server-toolkit --all-features` | ❌ Wave 0 (doctests written alongside code) |
| (CLAUDE.md ALWAYS) | Fuzz target on config.toml parser, ≥60s no panics | fuzz | `cargo +nightly fuzz run pmcp_server_toolkit_config_parser -- -max_total_time=60` | ❌ Wave 0 (new fuzz target; mirrors Phase 77 disposition: lands source even if local stable can't build) |
| (CLAUDE.md ALWAYS) | One runnable example demonstrating real usage | example | `cargo run -p pmcp-server-toolkit --example e01_toolkit_minimal --all-features` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p pmcp-server-toolkit --all-features` (unit + property + integration; ~5–10 sec on modern hardware)
- **Per wave merge:** `make quality-gate` (full CI-equivalent — fmt --all --check, clippy with pedantic+nursery, build, all test-* targets, audit)
- **Phase gate:** Full suite green before `/gsd-verify-work` — including `cargo +nightly fuzz run pmcp_server_toolkit_config_parser -- -max_total_time=60` and `cargo test --doc --all-features`

### Wave 0 Gaps

All test files are net-new. Wave 0 of plans should create:

- [ ] `crates/pmcp-server-toolkit/tests/backend_core_smoke.rs` — D-03 anchor; covers TKIT-08
- [ ] `crates/pmcp-server-toolkit/tests/reference_configs.rs` — D-17 integration anchor; covers TKIT-07 + REF-01 superset
- [ ] `crates/pmcp-server-toolkit/tests/tool_synthesis_props.rs` — proptest property anchor; covers TEST-02
- [ ] `crates/pmcp-server-toolkit/tests/code_mode_wiring.rs` — integration anchor; covers TKIT-06 + TKIT-09
- [ ] `crates/pmcp-server-toolkit/fuzz/Cargo.toml` + `fuzz_targets/pmcp_server_toolkit_config_parser.rs` — Phase 77 pattern; covers ALWAYS-fuzz
- [ ] `crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs` — ALWAYS-example
- [ ] `crates/pmcp-server-toolkit/fixtures/{open-images,imdb,msr-vtt}-config.toml` — operator-supplied snapshots OR copied from sibling pmcp-run repo
- [ ] Module-level unit tests in each `src/*.rs` (auth, secrets, resources, prompts, tools, code_mode, sql) — embedded `#[cfg(test)] mod tests {}` blocks (≥80% coverage per D-17)
- [ ] Doctests on every `pub` item — written inline with the code, not a separate file

*Existing infrastructure that covers nothing in this phase: `fuzz/fuzz_targets/pmcp_config_toml_parser.rs` lives at `cargo-pmcp/fuzz/` and targets `cargo_pmcp::test_support::configure_config::TargetConfigV1` — DIFFERENT schema. The toolkit needs its own fuzz target.*

## Security Domain

> Per CLAUDE.md "Contract-First Development" and CMSUP-02 threat model; toolkit re-exports security-sensitive types from pmcp-code-mode.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | Use `pmcp::AuthProvider` trait; toolkit's `StaticAuthProvider` for dev/test only — production uses pmcp's OAuth/JWT impls |
| V3 Session Management | no | The toolkit doesn't introduce sessions; pmcp's transport layer + AuthContext handle session shape |
| V4 Access Control | yes | `pmcp_code_mode::PolicyEvaluator` (NoopPolicyEvaluator default; AvpPolicyEvaluator for AWS) — re-exported via `toolkit::code_mode::*` |
| V5 Input Validation | yes | `#[serde(deny_unknown_fields)]` on every config section (D-13); optional `jsonschema` validation of tool args via `input-validation` feature; tool input schemas synthesized from `[[tools.parameters]]` |
| V6 Cryptography | yes | HMAC-SHA256 via `hmac = "0.13"` + `sha2 = "0.11"` (workspace-pinned); secret material in `secrecy::SecretBox<[u8]>` (zeroize-on-drop); never hand-roll |

### Known Threat Patterns for Rust crate / config-driven server

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Adversarial TOML triggers parser panic (DoS) | Denial of Service | `cargo +nightly fuzz run pmcp_server_toolkit_config_parser -- -max_total_time=60` — Phase 77 disposition pattern |
| Approval-token replay (T-CMSUP-02 from Phase 67.1) | Tampering / Repudiation | `pmcp_code_mode::HmacTokenGenerator` binds code-hash + request-id + ttl; never re-issue. Re-exported via `toolkit::code_mode` |
| Approval-secret leak via Debug/Clone/log | Information Disclosure | `TokenSecret` intentionally denies Debug/Clone/Serialize (verified at `crates/pmcp-code-mode/src/token.rs:40-46`) |
| Config typo silently widens policy (e.g., `auto_aprove_levels` parsed as None → defaults applied) | Tampering | D-13: `#[serde(deny_unknown_fields)]` rejects at parse time |
| SecretsProvider impl leaks secrets to logs | Information Disclosure | `SecretsProvider::get(name) -> Result<TokenSecret>` (NEVER returns `String` or `Vec<u8>` raw) — verify in lift |
| Tool dispatch bypasses code-mode policy | Elevation of Privilege | Code-mode's `validate_code` → `execute_code` flow requires approval token; `executor_from_config` enforces this via `ValidationPipeline` |
| Cross-crate version-drift between toolkit and pmcp-code-mode (silent API breakage) | Tampering | Workspace-version dep (D-05) — local path always wins until publish; CI builds with workspace resolver |

## Risks + Landmines

These come from CONTEXT.md spike findings + memory feedback notes:

1. **Code-mode + WASM incompatibility.** `pmcp-code-mode` pulls swc_ecma_parser (under `openapi-code-mode`) and sqlparser (under `sql-code-mode`). Both block WASM. **Mitigation:** D-14 drops both from the toolkit. Document explicitly in README. Toolkit does NOT support `wasm32-*` targets in 0.1.0.

2. **Phase 82 method semantics (`tool_arc` / `prompt_arc`).** `tool_arc` consumes `ToolInfo` from `handler.metadata()` (verified at `src/server/builder.rs:206-209`). If the synthesized handler returns `None` from `metadata()`, the builder falls back to an empty `ToolInfo`. **Mitigation:** `SynthesizedToolHandler::metadata()` MUST return `Some(ToolInfo)` with the full schema + annotations. Property test asserts this invariant.

3. **Dual-surface skill+prompt invariant (Phase 80 carryover).** Skills feature ships at pmcp 2.8.x with `skills` feature flag (gated `src/server/skills.rs`). Toolkit's StaticPromptHandler must NOT silently break the `bootstrap_skill_and_prompt` invariant — i.e., if a downstream consumer registers a Skill via pmcp's builder AND uses toolkit's StaticPromptHandler, the prompt-body-byte-equality-vs-SKILL.md invariant must still hold. **Mitigation:** Toolkit's StaticPromptHandler doesn't touch skills; they're orthogonal surfaces. Phase 87 will use the toolkit + skills together — that's where the invariant matters. Document the orthogonality in `prompts.rs` rustdoc.

4. **pmcp-run is a sibling repo, not visible from this tree.** All references to `pmcp-run/built-in/shared/mcp-server-common/` are EXTERNAL inputs (canonical_refs §"Lift Source"). The actual files are not in this repo. **Mitigation:** Plans must list "Operator/agent retrieves mcp-server-common/src/{auth,secrets,resources,prompts}.rs from sibling repo and copies into crates/pmcp-server-toolkit/src/" as an explicit early task. If sibling repo not accessible, plan owner blocks on operator-supplied snapshots.

5. **PMAT cog-25 ceiling on synthesizer.** TKIT-07's synthesizer is the most complex new code (~200 LoC). Decompose into ≤3 helpers each ≤cog 25 (per Phase 75 D-03).

6. **`pmcp = { default-features = false }` in toolkit Cargo.toml.** pmcp's `default = ["logging"]` pulls in tracing-subscriber transitively. Toolkit consumers may or may not want this. **Mitigation:** Use `default-features = false` and explicitly declare needed pmcp features (e.g., `features = []` or `features = ["validation"]` if `input-validation` is enabled). Document in plan: feature composition between toolkit features and pmcp features.

7. **The "code-hash binding" in HMAC tokens.** `pmcp_code_mode::hash_code` + `canonicalize_code` + `ApprovalToken` bind code → token. The toolkit MUST not re-implement; re-export only. **Mitigation:** D-16 mandates re-export. Verify in plan-check.

8. **Trait-stub-in-P83-vs-impls-in-P84.** TKIT-10 requires `SqlConnector` trait stub + `Dialect` enum stub in toolkit core. If P83 omits the trait, P84 has nowhere to plug in. If P83 over-specifies, P84 may be forced to refactor. **Mitigation:** Follow spike 005 verbatim — 3 methods (`dialect`, `execute`, `schema_text`) + `Dialect` enum with 4 variants (`Postgres`, `MySql`, `Athena`, `Sqlite`) + 2 free helpers (`translate_placeholders`, `build_code_mode_prompt`). P83 ships the trait + enum + helpers + MockSqlConnector for tests. P84 ships per-backend impls.

9. **Crate 10MB publish limit.** Toolkit must exclude `.planning/`, `.pmat/`, `fixtures/` from the published artifact. **Mitigation:** Cargo.toml `exclude = [...]` whitelist. Verify with `cargo package --list`.

## Phase-Split Risk Assessment

**Phase 83 has 12 requirement IDs, 5 success criteria, ~2500–3000 LoC of new+lifted code, and 8 distinct source files (4 lift + 4 net-new). Should it be split?**

**Verdict: KEEP AS ONE PHASE, plan as ~8 plans.**

**Rationale:**

1. **Tight artifact cohesion.** All 12 reqs land in one crate. Splitting (e.g., "P83a = lift", "P83b = synthesis", "P83c = code-mode") would either ship a broken crate at each phase boundary (lift without ServerConfig is meaningless) or require multiple version bumps.

2. **CONTEXT.md decisions assume single phase.** D-01, D-08, D-17 all reference one publishable artifact at `0.1.0`. Splitting would require a multi-phase release strategy that's not in scope.

3. **Spike 003 + spike 004 + spike 005 already de-risked the work.** The proto-SDK is already extracted; the trait shape is already proven; the 12-line user surface is already validated in-binary. P83 is a lift + 3 net-new pieces, not net-new exploration.

4. **STATE.md sets the precedent.** "Anchor phases: Phase 83 (TKIT, 12 reqs) and Phase 84 (CONN, 10 reqs) are the two intentionally-large 'lift the proto-SDK' phases." The operator already accepted P83 size at roadmap creation.

**Recommended plan breakdown (~8 plans):**

| # | Plan focus | Reqs | LoC est. | Depends on |
|---|------------|------|----------|------------|
| 1 | Crate scaffold + Cargo.toml + workspace insertion + CLAUDE.md publish-order edit + CRATE-README | TKIT-01 | ~150 | — |
| 2 | `ServerConfig` + all sub-structs + `from_toml` + `deny_unknown_fields` everywhere + REF-01 superset enumeration (consume + parse 3 reference fixtures) | TKIT-01, REF-01 anchor (toolkit-side) | ~400 | Plan 1 |
| 3 | Lift `auth.rs` + `secrets.rs` from mcp-server-common (verbatim) + unit tests + doctests | TKIT-02, TKIT-03 | ~1300 | Plan 1 |
| 4 | Lift `resources.rs` + `prompts.rs` from mcp-server-common (verbatim) + unit tests + doctests + integrate `StaticResourceHandler::from(&ServerConfig)` constructor | TKIT-04, TKIT-05 | ~700 | Plan 2 |
| 5 | `code_mode.rs` re-exports + `code_mode_from_config` + `executor_from_config` + integration test for `[code_mode]` policy enforcement | TKIT-06, TKIT-09 | ~250 | Plan 2 |
| 6 | `tools.rs` synthesizer (low-level fn) + `SynthesizedToolHandler` + property test on round-trip | TKIT-07 + TEST-02 | ~300 | Plan 2 |
| 7 | `sql/mod.rs` `SqlConnector` trait stub + `Dialect` enum stub + `translate_placeholders` helper + `MockSqlConnector` for tests + `code_mode::assemble_code_mode_prompt` | TKIT-10 + TEST-02 (placeholder property) | ~250 | Plan 6 |
| 8 | `ServerBuilderExt` trait + `.tools_from_config` + `.code_mode_from_config` builder methods + integration smoke test (`backend_core_smoke.rs`) + reference configs test (`reference_configs.rs`) + fuzz target + example + final ALWAYS gate + shim diff artifact + final verification | TKIT-08, TEST-02, TEST-03 (final) | ~350 | All prior |

**Parallelism note:** Plans 3, 4, 5, 6, 7 can largely run in parallel once Plans 1 and 2 land (config + scaffold).

## Sources

### Primary (HIGH confidence)

- `.planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/83-CONTEXT.md` — Locked decisions D-01..D-17, scope boundaries, canonical refs (read in full)
- `.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md` — Auto-loaded spike skill; requirements baseline, implementation order, source URLs
- `.claude/skills/spike-findings-rust-mcp-sdk/references/schema-server-architecture.md` — Proto-SDK already extracted, no SchemaServer trait, three user-facing shapes, upstream DX gaps, anti-patterns
- `.claude/skills/spike-findings-rust-mcp-sdk/references/schema-server-sql-dialects.md` — `SqlConnector` 3-method trait + `Dialect` 4-variant enum + `translate_placeholders` + `build_code_mode_prompt`; 3-step extension protocol
- `.planning/REQUIREMENTS.md` §"Toolkit Core" — TKIT-01..10 full text + TEST-02/03 toolkit-coverage requirements (lines 125–138, 202–203)
- `.planning/ROADMAP.md` Phase 83 block (lines 1396–1409) — 5 success criteria verbatim
- `src/types/tools.rs:1-270` — Verified `ToolInfo`, `ToolAnnotations`, `ToolInfo::new`, `ToolInfo::with_annotations`, all annotation fields used by `[tools.annotations]`
- `src/server/builder.rs:200-450` — Verified `tool_arc`, `prompt_arc`, `resources_arc`, `auth_provider_arc` exist on public `ServerBuilder`
- `src/server/auth/traits.rs:447-474` — Verified `AuthProvider` trait shape
- `src/server/mod.rs:222-282` — Verified `ToolHandler`, `PromptHandler`, `ResourceHandler` trait shapes
- `crates/pmcp-code-mode/src/lib.rs:1-213` — Verified all re-exported types (CodeExecutor, TokenSecret, HmacTokenGenerator, NoopPolicyEvaluator, AvpPolicyEvaluator, CodeModeConfig, etc.) per D-16
- `crates/pmcp-code-mode/src/code_executor.rs:55-67` — Verified `CodeExecutor::execute(code, variables)` signature
- `crates/pmcp-code-mode/src/config.rs:95-300` — Verified `CodeModeConfig` field shape including `alias = "allow_writes"` etc.
- `crates/pmcp-code-mode/src/token.rs:1-100` — Verified `TokenSecret` (secrecy + zeroize), `HmacTokenGenerator`, code-hash binding pattern
- `crates/mcp-tester/Cargo.toml:21` — Verified workspace-version dep pattern: `pmcp = { version = "2.8.1", path = "../../", features = [...] }`
- `Cargo.toml:540-543` — Workspace members list, insertion point for toolkit
- `.planning/phases/82-builder-dx-prerequisites/82-VERIFICATION.md:25-30` — Phase 82 verification — all 6 `_arc` methods landed as `pub fn`; `get_tool` + `get_prompt` accessors land
- `cargo-pmcp/fuzz/fuzz_targets/pmcp_config_toml_parser.rs` — Phase 77 fuzz target pattern to mirror for toolkit's own parser fuzz target

### Secondary (MEDIUM confidence)

- `crates/pmcp-code-mode/Cargo.toml` — Verified feature shapes (`openapi-code-mode`, `sql-code-mode`, `js-runtime`, `mcp-code-mode`, `cedar`, `avp`); confirms which are dropped from toolkit per D-14
- `src/server/skills.rs:1-80` — Verified IndexMap usage pattern + `Content::resource_with_text` MIME-typed wire shape
- `cargo search hmac/secrecy/rusqlite/jsonschema` — Current crates.io versions (2026-05-18): `hmac@0.13.0`, `secrecy@0.10.3`, `rusqlite@0.39.0`, `jsonschema@0.46.5`
- `fuzz/Cargo.toml:1-100` — Confirmed cargo-fuzz convention (pmcp's own fuzz workspace structure; toolkit can mirror with its own fuzz workspace)

### Tertiary (LOW confidence)

- Exact LoC counts in `pmcp-run/built-in/shared/mcp-server-common/src/{secrets,resources,prompts,auth}.rs` — quoted as 701 / 333 / 285 / ~600 from CONTEXT.md canonical_refs; not directly verifiable from this repo
- Exact content of the three reference config.tomls (open-images, imdb, msr-vtt) — referenced but not in this repo; A4 in Assumptions Log marks this MEDIUM risk

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH — every dep verified against workspace state or cargo search 2026-05-18
- Architecture: HIGH — CONTEXT.md locks the design; spike findings + verified pmcp source files corroborate every decision
- Pitfalls: HIGH — derived from real workspace patterns (Phase 75 cog ceiling, Phase 77 fuzz pattern, Phase 82 builder shape) + concrete file references
- Phase split: HIGH — STATE.md explicitly precedented this phase as "intentionally large"; 8-plan breakdown maps cleanly to single-file boundaries

**Research date:** 2026-05-18
**Valid until:** 2026-06-15 (workspace state is stable; only Phase 84 changes the toolkit dependency surface, but P83 is upstream of P84)
