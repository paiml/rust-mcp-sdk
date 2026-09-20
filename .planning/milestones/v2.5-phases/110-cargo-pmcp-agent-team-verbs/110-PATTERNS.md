# Phase 110: cargo-pmcp Agent & Team Verbs - Pattern Map

**Mapped:** 2026-07-19
**Files analyzed:** 18 (9 new source + 4 modified config/barrel + 5 new tests)
**Analogs found:** 18 / 18 (every new file has a concrete in-repo analog — this is a thin-CLI-over-shipped-crates phase)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `cargo-pmcp/src/commands/agent/mod.rs` | route (command-group) | request-response | `cargo-pmcp/src/commands/workbook/mod.rs` | exact |
| `cargo-pmcp/src/commands/agent/new.rs` | controller (scaffolder) | file-I/O | `cargo-pmcp/src/commands/new.rs::execute_sql_server` | exact |
| `cargo-pmcp/src/commands/agent/dev.rs` | controller | event-driven (async loop) | `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs` | exact (runtime), role-match (CLI shell) |
| `cargo-pmcp/src/commands/team/mod.rs` | route (command-group) | request-response | `cargo-pmcp/src/commands/workbook/mod.rs` | exact |
| `cargo-pmcp/src/commands/team/dev.rs` | controller | event-driven (async orchestration) | `crates/pmcp-team-servers/examples/doc_review_team.rs` | exact (runtime), role-match (CLI shell) |
| `cargo-pmcp/src/commands/package/mod.rs` | route (command-group) | request-response | `cargo-pmcp/src/commands/workbook/mod.rs` | exact |
| `cargo-pmcp/src/commands/package/show.rs` | controller | file-I/O (offline transform) | `crates/pmcp-package/src/oci/{layout.rs,unpack.rs,mod.rs}` | role-match (new consumer of existing API) |
| `cargo-pmcp/src/commands/package/capture.rs` | controller | request-response (authenticated HTTP) | `configure::resolver` + `auth_cmd::cache` | role-match |
| `cargo-pmcp/src/templates/agent.rs` | template (scaffolder + pin tripwire) | file-I/O + transform | `cargo-pmcp/src/templates/workbook_server.rs` | exact |
| `cargo-pmcp/src/main.rs` (modify) | config (entrypoint) | request-response | `cargo-pmcp/src/main.rs` (`Workbook`/`Test` arms) | exact (self) |
| `cargo-pmcp/Cargo.toml` (modify) | config | — | existing `pmcp` dep line + `Cargo.toml` pin-drift precedent | role-match |
| `cargo-pmcp/src/commands/mod.rs` (modify) | config (barrel) | — | existing `pub mod workbook;` lines | exact |
| `cargo-pmcp/src/templates/mod.rs` (modify) | config (barrel) | — | existing `pub mod workbook_server;` lines | exact |
| `cargo-pmcp/tests/scaffold_agent.rs` | test | file-I/O | `cargo-pmcp/tests/scaffold_sql_server.rs` | exact |
| `cargo-pmcp/tests/agent_dev.rs` | test | event-driven | `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs::run_standalone` | role-match |
| `cargo-pmcp/tests/team_dev.rs` | test | event-driven | `crates/pmcp-team-servers/examples/doc_review_team.rs` | role-match |
| `cargo-pmcp/tests/package_show.rs` | test | file-I/O | `cargo-pmcp/tests/scaffold_sql_server.rs` (CLI acceptance shape) | role-match |
| `cargo-pmcp/tests/package_capture.rs` | test | request-response | `cargo-pmcp/tests/cli_acceptance.rs` | role-match |

## Pattern Assignments

### `cargo-pmcp/src/commands/{agent,team,package}/mod.rs` (route, request-response)

**Analog:** `cargo-pmcp/src/commands/workbook/mod.rs` (whole file, 130 lines — read it once, mirror it three times)

**Module + subcommand-enum + dispatch pattern** (`workbook/mod.rs:21-98`):
```rust
pub mod compile;   // one `pub mod <sub>;` per subcommand handler file
pub mod lint;

use anyhow::Result;
use clap::Subcommand;
use super::GlobalFlags;

/// `cargo pmcp workbook <subcommand>` — the command group (D-04).
#[derive(Debug, Subcommand)]
pub enum WorkbookCommand {
    /// Compile a workbook into a gated, served bundle.
    Compile(compile::CompileArgs),
    Lint(lint::LintArgs),
}

impl WorkbookCommand {
    /// Dispatch the subcommand to its handler.
    pub fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            WorkbookCommand::Compile(args) => compile::execute(args, global_flags),
            WorkbookCommand::Lint(args) => lint::execute(args, global_flags),
        }
    }
}
```

**Per-file mapping for Phase 110:**
- `agent/mod.rs` → `enum AgentCommand { New(new::NewArgs), Dev(dev::DevArgs) }`
- `team/mod.rs` → `enum TeamCommand { Dev(dev::DevArgs) }`
- `package/mod.rs` → `enum PackageCommand { Capture(capture::CaptureArgs), Show(show::ShowArgs) }`

**CRITICAL deviation from the analog:** `workbook`'s `execute` is **sync** (`-> Result<()>`). `agent dev`, `team dev`, and `package capture` are **async**. The dispatch signature becomes `pub async fn execute(self, global_flags: &GlobalFlags) -> Result<()>` and each async arm is `.await`ed. Follow the async groups (`Loadtest`/`Auth` at `main.rs:559-579`) for how `main.rs` already wires `block_on` (see `main.rs:643` `runtime.block_on(command.execute(...))`).

**Optional: typed exit codes** — `workbook/mod.rs:35-73` defines `EXIT_OK/EXIT_ERROR/EXIT_GATE_BLOCK` constants + a `WorkbookExit` typed error that `main.rs` downcasts. Phase 110 likely does NOT need distinct exit codes (plain `anyhow::bail!` → exit 1 suffices for the actionable-error paths in D-03a/D-04a). Skip unless a plan needs to distinguish "unconfigured" from "failed".

---

### `cargo-pmcp/src/commands/agent/new.rs` (controller/scaffolder, file-I/O)

**Analog:** `cargo-pmcp/src/commands/new.rs::execute_sql_server` (lines 149-170)

**Scaffolder handler pattern** (`new.rs:149-170`) — validate name BEFORE any fs write, create `src/`, delegate to a `templates::*::generate`:
```rust
fn execute_sql_server(
    workspace_dir: &Path,
    name: &str,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    // Validate the crate name BEFORE any fs::write (Codex MEDIUM / T-86-03-02).
    validate_crate_name(name)?;
    fs::create_dir_all(workspace_dir.join("src")).context("Failed to create src directory")?;
    templates::sql_server::generate(workspace_dir, name)?;
    if global_flags.should_output() {
        println!("\n{} SQL server crate created successfully!", "✓".green().bold());
        print_sql_server_next_steps(name);
    }
    Ok(())
}
```

**Reuse `validate_crate_name`** (`new.rs:119-140`) — do NOT re-implement. It is the hardened path-traversal + Cargo-name guard (rejects empty, leading digit, `/`, `\`, `..`, illegal chars). It is currently a private `fn` in `new.rs`; the planner must decide whether to make it `pub(crate)` and call it from `agent/new.rs`, or route `agent new` through `commands::new`. **Recommendation:** promote to `pub(crate) fn validate_crate_name` and call it — Don't-Hand-Roll (RESEARCH §Don't Hand-Roll).

**`--kind` dispatch precedent** (`new.rs:70-83`) shows how `execute_sql_server`/`execute_workbook_server` are selected — but D-01 forbids adding an `agent` `--kind`; `agent new` is surfaced under the `agent` group instead, calling the same scaffolder shape internally (D-01a).

---

### `cargo-pmcp/src/templates/agent.rs` (template + pin tripwire, file-I/O + transform)

**Analog:** `cargo-pmcp/src/templates/workbook_server.rs` (whole file, 511 lines — the canonical scaffold-emitter + pin-drift-guard)

**Emitter orchestrator + per-file `generate_*`** (`workbook_server.rs:61-102`):
```rust
pub fn generate(dir: &Path, name: &str) -> Result<()> {
    generate_cargo_toml(dir, name)?;
    generate_main_rs(dir)?;
    // ... one generate_<file> per output file, each a raw fs::write(...).context(...)
    Ok(())
}

fn generate_cargo_toml(dir: &Path, name: &str) -> Result<()> {
    let content = format!(r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
pmcp = {{ version = "{PMCP_VERSION}", features = ["streamable-http"] }}
"#);
    fs::write(dir.join("Cargo.toml"), content).context("Failed to create Cargo.toml")?;
    Ok(())
}
```
There is NO template engine — raw string literals via `format!` (escape literal braces as `{{`/`}}`).

**Pin-drift tripwire — THE core D-05 mechanism** (`workbook_server.rs:51-59` const + `470-510` test):
```rust
/// The pinned version the emitted Cargo.toml declares. A test asserts this equals
/// the workspace crate's [package] version so the hardcoded pin cannot silently drift.
const PMCP_VERSION: &str = "2.17.0";

#[test]
fn emitted_pmcp_version_matches_workspace_pin() {
    const ROOT_CARGO_TOML: &str = include_str!("../../../Cargo.toml");
    let parsed: toml::Value = toml::from_str(ROOT_CARGO_TOML).expect("parse");
    let root_version = parsed
        .get("package").and_then(|p| p.get("version")).and_then(|v| v.as_str())
        .expect("root Cargo.toml has [package] version");
    assert_eq!(
        PMCP_VERSION, root_version,
        "the scaffold's hardcoded pmcp version drifted from the workspace pin — bump PMCP_VERSION"
    );
}
```

**Phase 110 application (D-05, two distinct tripwires):**
1. `templates/agent.rs` carries `const PMCP_AGENT_VERSION: &str = "0.1.0";` + `emitted_agent_version_matches_workspace_pin` test that `include_str!("../../../crates/pmcp-agent/Cargo.toml")` and asserts `["package"]["version"]` matches (mirror the excerpt above exactly, retargeting the path).
2. Per Open-Q1 (RESEARCH), also emit a `tests/pin.rs` INTO the agent scaffold asserting its `Cargo.toml` pins `pmcp-agent` — ship both (safest reading of "generated tripwire test").
3. CLI-04: a **cargo-pmcp-internal** test (in `templates/agent.rs` or a small `tests/` unit) asserting `cargo-pmcp/Cargo.toml`'s `pmcp-package` dep line is caret `"0.1"` — parse cargo-pmcp's own `Cargo.toml`, assert the `pmcp-package` version req string == `"0.1"`.

**AgentPackage manifest the scaffold must emit** — the exact struct-literal shape is in `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs:62-83` (`agent_package()`), reproduced verbatim below in the Shared Patterns section (it is load-bearing — the emitted JSON must round-trip through `pmcp_package::AgentPackage`).

**Golden/drift-lock precedent (optional):** `workbook_server.rs:211-320` shows a `wiring_lines`-normalizing golden test that proves the emitted `src/main.rs` cannot drift from a canonical example. If `agent new`'s runner is lifted from `s50_standalone_vs_sampled.rs`, mirror this to drift-lock against the example.

---

### `cargo-pmcp/src/commands/agent/dev.rs` (controller, event-driven async loop)

**Analog:** `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs` (the canonical two-mode demo — 380 lines, read once)

**Imports** (`s50:44-49`):
```rust
use pmcp_agent::{
    AgentEngine, AgentServer, CompletionError, CompletionSource, CompletionSourceFactory,
    InMemoryStore, ResolvedAgentConfig, RunOutcome, SamplingSourceFactory, ToolCall,
    ToolCallResult, ToolInvoker,
};
use pmcp_agent::sources::{OpenAiCompatSource, SecretString}; // feature = "openai-compat"
```

**`--source openai-compat|fixed` path — engine over a source** (`s50:52-59,187-204,286-305`):
```rust
let config = ResolvedAgentConfig::new(
    "You are a concise assistant.", "llama3.2", 100_000, 5,
);
let source = OpenAiCompatSource::new(
    "http://localhost:11434/v1", "llama3.2", SecretString::new("ollama"),
)?; // D-03a: map the first-.run() transport error to an actionable message naming --endpoint/--source
let engine = AgentEngine::new(source, invoker, InMemoryStore::new(), config);
let outcome: RunOutcome = engine.run("agent-dev-run").await;
```
`fixed` mode = inject a scripted/end-turn `CompletionSource` (see `ScriptedSource` `s50:99-136`) via `FixedSourceFactory` — offline/CI, no external LLM.

**`--source sampling` path — agent-as-server, host provides the LLM** (`s50:207-234`):
```rust
let factory: Arc<dyn CompletionSourceFactory> = Arc::new(SamplingSourceFactory::new());
let agent = AgentServer::builder(
    agent_package(), agent_config(), factory,
    Arc::new(DemoInvoker::default()), Arc::new(InMemoryStore::new()),
).build()?;
let tool_name = agent.tool_name().to_string();
// agent.run(transport).await — native-only; run over stdio/HTTP transport
```

**Invoker** — for a real run use `pmcp_agent::invoker::ClientToolInvoker`; for a minimal demo a no-op/echo invoker suffices (`DemoInvoker` at `s50:85-97` records + echoes ok). Per Open-Q3, the first pass may run a no-op invoker (planner's call).

**Actionable-endpoint-error (D-03a / Pitfall 4):** catch `CompletionError::Transport` from the first completion and re-emit naming `--endpoint`/`--source fixed`. Consider a short connect timeout so a missing Ollama fails fast instead of hanging.

---

### `cargo-pmcp/src/commands/team/dev.rs` (controller, event-driven async orchestration)

**Analog:** `crates/pmcp-team-servers/examples/doc_review_team.rs` (THE reference transcript — 406 lines, read once) + `crates/pmcp-team-servers/tests/small_team.rs`

**Imports** (`doc_review_team.rs:39-50`):
```rust
use pmcp_agent::{
    CompletionError, CompletionSource, CompletionSourceFactory, FixedSourceFactory,
    ProgrammaticBuilder, SlotResolver,
};
use pmcp_package::package::team::{HumanRole, TeamLimits, TeamMember, TeamRole};
use pmcp_package::{AgentPackage, ComponentRef, ConfigSlot, TeamPackage};
use pmcp_team_servers::compose::resolver::LocalDirPackageResolver;
use pmcp_team_servers::compose::wiring::TeamRuntimeBuilder;
```

**Build the in-process TeamRuntime with a FixedSource override** (`doc_review_team.rs:79-83,227-246`):
```rust
fn fixed_override() -> Arc<dyn CompletionSourceFactory> {
    Arc::new(FixedSourceFactory::new(Arc::new(EndTurnMock) as Arc<dyn CompletionSource>))
}

let rt = TeamRuntimeBuilder::new(resolver, stub_slot_resolver())
    .with_completion_override(fixed_override())   // offline default (D-02b)
    .with_data_root(data_dir.path())
    .build(&pkg)                                  // &TeamPackage
    .await?;

let team_fs  = rt.team_fs_client().expect("team-fs attached (opt-in)");
let approval = rt.approval_client().expect("approval-mcp attached (1 human)");
let mem      = rt.mem_client().expect("mem-mcp attached (opt-in)");
let team_mcp = rt.team_mcp_client().expect("team-mcp attached (members)");
```

**The labeled-transcript flow (D-02 default output)** — the exact 7-step ordering the CLI prints (`doc_review_team.rs:250-398`):
1. `fs__write` — drafter writes the doc (`:256`)
2. `fs__sync_to_review` — publish for review (`:274`)
3. discover `team_approval__ask_<role>` via `approval.list_tools`, then call it linking `subjectRef` (`:281-306`)
4. `resolve_approval` — human verdict (`:323`)
5. `fs__read` — summarizer reads approved doc (`:340`)
6. `mem__add` — store the summary (`:353`)
7. discover `team_mcp__<member>` via `team_mcp.list_tools`, dispatch, surface `RELATED_TASK_META_KEY` from `._meta` (`:371-398`)

The `body()` helper (`:185-192`) extracts the JSON from `CallToolResult.content[0]`, and `step(n, msg)` (`:194-197`) prints the labeled header — copy both for the transcript formatting (D-02 discretion allows relabeling/coloring).

Clean shutdown: `rt.shutdown().await` returns the joined hosting-task count (`:401`).

**`--serve` (D-02a):** build `pmcp-team-servers` with the `http` feature and take the HTTP-first team-mcp binary path (109 D-04) instead of driving the flow in-process. **`--llm` (D-02b):** replace `with_completion_override(fixed_override())` with a real `OpenAiCompatSource`-backed factory (needs the `member-llm` feature).

---

### `cargo-pmcp/src/commands/package/show.rs` (controller, file-I/O offline transform)

**Analog:** `crates/pmcp-package/src/oci/{layout.rs,unpack.rs,mod.rs}` (the API this new file consumes)

**OCI open + read-index + media-type dispatch** — a `.pmcp` package is an OCI image-layout DIRECTORY (`blobs/sha256/` + `index.json`):
```rust
use pmcp_package::oci::{OciLayout, unpack_agent, unpack_team, unpack_server, unpack_workflow};

let layout = OciLayout::open(path);   // infallible open (layout.rs:63)
let index = layout.read_index()?;     // ImageIndex → manifest descriptor (layout.rs:110)
// Read the manifest's artifactType / config layer media type to pick the kind,
// then dispatch to the matching unpack_* (unpack.rs:141-151):
let agent: AgentPackage = unpack_agent(&layout)?;             // MT_AGENT_CONFIG
// unpack_team -> TeamPackage; unpack_server -> (ServerPackage, Vec<u8>);
// unpack_workflow -> WorkflowManifest
```

**Media-type constants for kind detection** (`oci/media_types.rs:60-77`) — read these to dispatch deterministically (Pitfall 3 — do NOT try each `unpack_*` in turn):
```
MT_AGENT_CONFIG    = "application/vnd.pmcp.agent.config.v1+json"     ARTIFACT_TYPE_AGENT
MT_TEAM_CONFIG     = "application/vnd.pmcp.team.config.v1+json"      ARTIFACT_TYPE_TEAM
MT_SERVER_ENVELOPE = "application/vnd.pmcp.mcp-server.envelope..."   ARTIFACT_TYPE_SERVER
MT_WORKFLOW_MANIFEST = "application/vnd.pmcp.workflow.manifest..."   ARTIFACT_TYPE_WORKFLOW
```
The `SingleLayerPackage` trait (`oci/mod.rs:40-87`) binds each kind → its media/artifact constants; use `ARTIFACT_TYPE_*` off the manifest for the dispatch switch. Provide a clear "unknown package kind" error otherwise.

This path is fully offline — no platform, no network. Digest verification happens inside `unpack_*` (V6/Cryptography delegated); surface verification failures, never bypass.

---

### `cargo-pmcp/src/commands/package/capture.rs` (controller, authenticated HTTP)

**Analog:** `cargo-pmcp/src/commands/configure/resolver.rs` + `cargo-pmcp/src/commands/auth_cmd/cache.rs` (reuse — D-04a forbids new config)

**Resolve the active platform target** (`configure/resolver.rs:179` + `ResolvedTarget::api_url` at `:109`):
```rust
use crate::commands::configure::resolver::{resolve_active_target_name, resolve_target};
// resolve_target(explicit_name, cli_flag, project_root, deploy_config) -> Result<Option<ResolvedTarget>>
let target = resolve_target(None, cli_flag, project_root, deploy_config)?
    .ok_or_else(|| anyhow::anyhow!(
        "no platform target configured — run `cargo pmcp configure add <name>` first"))?;
let api_url = target.api_url()   // Option<&ResolvedField> (resolver.rs:109)
    .ok_or_else(|| anyhow::anyhow!("target has no api_url — set it via `cargo pmcp configure`"))?;
```

**Read the cached Bearer token** (`auth_cmd/cache.rs:68,128,139`):
```rust
use crate::commands::auth_cmd::cache::{TokenCacheV1, default_multi_cache_path, normalize_cache_key};
let cache = TokenCacheV1::read(&default_multi_cache_path())?;   // cache.rs:68
let key = normalize_cache_key(api_url_str)?;                    // cache.rs:139
let entry = cache.get(&key).ok_or_else(|| anyhow::anyhow!(
    "not authenticated for {api_url_str} — run `cargo pmcp auth login {api_url_str}`"))?;
// entry: TokenCacheEntry (cache.rs:34); is_near_expiry(entry, REFRESH_WINDOW_SECS) at :163
```

**Authenticated POST** — Bearer-header shape is established at `commands/auth.rs:151-174` (`format!("Bearer {token}")`) and `pentest/attacks/auth_flow.rs:408` (`.header("Authorization", format!("Bearer {jwt}"))`). Use `reqwest` (already a cargo-pmcp dep) to POST the packed package to the configured `{api_url}/…capture` path.

**A1/Open-Q2 — the platform capture-API endpoint is out-of-repo (platform-owned).** Scope `capture` to: resolve target + token → POST to a `{api_url}`-relative capture path (make the path a named constant the planner can confirm) → **degrade to an actionable error when unconfigured, NEVER a panic or silent stub** (D-04a). Flag the exact endpoint as a platform-coordination item; it does not block the config/auth/error-handling work.

**Security (V2/token leakage):** never `println!` a token; reuse the redaction already in the tree (the agent `SecretString` at `s50:273-278` redacts).

---

### `cargo-pmcp/src/main.rs` (modify — config/entrypoint)

**Analog:** self — the existing `Workbook` arm (`main.rs:169-172`) and async dispatch (`main.rs:559-579`)

**Add three `enum Commands` arms** (mirror `main.rs:169-172`):
```rust
/// Scaffold and run agents (AgentPackage-backed)
Agent { #[command(subcommand)] command: commands::agent::AgentCommand },
/// Run an in-process small team from a TeamPackage
Team { #[command(subcommand)] command: commands::team::TeamCommand },
/// Capture/show portable .pmcp packages
Package { #[command(subcommand)] command: commands::package::PackageCommand },
```

**Add three dispatch arms** (near `main.rs:559-580`). `Workbook` is sync; these are async — follow the `block_on` wiring already used for async groups (`main.rs:643` shows `runtime.block_on(command.execute(...))`):
```rust
Commands::Agent { command }   => runtime.block_on(command.execute(global_flags)),
Commands::Team { command }    => runtime.block_on(command.execute(global_flags)),
Commands::Package { command } => runtime.block_on(command.execute(global_flags)),
```

**Note `is_target_consuming`** (`main.rs:374-382`): `package capture` consumes a configure target — the planner should decide whether to add `Commands::Package { .. }` there (so `PMCP_TARGET`/AWS env side-effects apply) or resolve the target explicitly inside `capture.rs`. `capture.rs` resolves via `resolve_target` directly, so it likely does NOT need to be target-consuming — confirm at plan time.

---

### `cargo-pmcp/Cargo.toml` (modify — config)

**Analog:** the existing `pmcp` dep line + the `Cargo.toml` version-drift precedent (`workbook_server.rs:471-510`)

Add (RESEARCH §Installation — feature flags are load-bearing, Pitfall 1):
```toml
[dependencies]
pmcp-agent = { version = "0.1", path = "../crates/pmcp-agent", features = ["openai-compat"] }
pmcp-team-servers = { version = "0.1", path = "../crates/pmcp-team-servers", features = ["runtime", "http", "member-llm"] }
pmcp-package = { version = "0.1", path = "../crates/pmcp-package" }
```
`pmcp-package` MUST be caret `"0.1"` (CLI-04/D-04b) — a cargo-pmcp-internal tripwire test asserts this exact string. Publish-order: cargo-pmcp moves after all three (design §5) — `version = "0.1" + path` (path wins locally, version applies at publish; Pitfall 2).

---

### `cargo-pmcp/src/commands/mod.rs` + `cargo-pmcp/src/templates/mod.rs` (modify — barrels)

Add `pub mod agent;`, `pub mod team;`, `pub mod package;` to `commands/mod.rs` and `pub mod agent;` to `templates/mod.rs` — mirror the existing `pub mod workbook;` / `pub mod workbook_server;` lines exactly.

---

### Test files (new)

**`cargo-pmcp/tests/scaffold_agent.rs`** (CLI-01) — **Analog: `cargo-pmcp/tests/scaffold_sql_server.rs`.** Invoke the REAL built binary via `env!("CARGO_BIN_EXE_cargo-pmcp")` in a `tempfile::tempdir()` (do NOT weaken to an in-process `execute` call — see `scaffold_sql_server.rs:1-12`). Reuse `tests/support/scaffold_patch.rs` (`append_crates_io_patch`, `ChildGuard`) for the `[patch.crates-io]` block resolving unpublished workspace deps (`scaffold_sql_server.rs:51-56`). Assert the emitted AgentPackage manifest + runner + `tests/pin.rs` exist. Must run `--test-threads=1` (heavy tempdir build).

**`cargo-pmcp/tests/agent_dev.rs`** (CLI-02) — offline `--source fixed` run; mirror `s50::run_standalone` (`s50:187-204`) — a scripted `CompletionSource` + no-op invoker + `AgentEngine::run`, assert `RunOutcome::Completed`.

**`cargo-pmcp/tests/team_dev.rs`** (CLI-03) — offline transcript assertion; mirror `doc_review_team.rs` end-to-end (build `TeamRuntime` with `fixed_override()`, run the 7 steps, assert the labeled lines / `rt.shutdown()` count).

**`cargo-pmcp/tests/package_show.rs`** (CLI-04) — build a small `.pmcp` fixture via `pmcp_package::oci::pack_agent` (or a committed fixture dir), then assert `show` renders it offline. CLI-acceptance shape from `cargo-pmcp/tests/cli_acceptance.rs`.

**`cargo-pmcp/tests/package_capture.rs`** (CLI-04) — unconfigured → actionable error (assert exit + stderr names `configure`/`auth`); `assert_cmd`/`predicates` shape from `cargo-pmcp/tests/cli_acceptance.rs`.

## Shared Patterns

### AgentPackage manifest literal (both `agent new` scaffold output AND `team dev` member fixtures)
**Source:** `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs:62-83` (also `doc_review_team.rs:101-124`)
**Apply to:** `templates/agent.rs` (emitted JSON manifest), `agent/dev.rs` sampling mode, `team/dev.rs` member fixtures, `tests/*`
```rust
AgentPackage {
    name: "research-agent".to_string(),
    version: semver::Version::parse("1.0.0").unwrap(),
    instructions: "...".to_string(),
    llm: ConfigSlot { slot: SlotType::LlmProvider {
        name: "primary-llm".to_string(), tested_value: "demo-model".to_string() } },
    max_tokens: 100_000, max_iterations: 5,
    connectors: vec![], tool_selection: None, input_schema: None,
    output_schema: None, importance: None, finalizer_role: None, budget_defaults: vec![],
}
```
The emitted scaffold manifest MUST round-trip through this exact struct (`serde_json`), or `pmcp-package` parse fails. `imports`: `use pmcp_package::{AgentPackage, ConfigSlot, SlotType};`.

### Version-pin drift tripwire (const + `include_str!` + `assert_eq!`)
**Source:** `cargo-pmcp/src/templates/workbook_server.rs:51-59, 470-510`
**Apply to:** `templates/agent.rs` (`PMCP_AGENT_VERSION`), cargo-pmcp-internal `pmcp-package = "0.1"` caret assertion
The full excerpt is in the `templates/agent.rs` assignment above. This is a **mandatory reuse** (D-05) — do NOT invent a new mechanism.

### Colored console output convention
**Source:** `new.rs:99-108`, `workbook_server.rs:69-71`, `doc_review_team.rs:194-197`
**Apply to:** all four verbs' output (next-steps, transcript, render)
`colored::Colorize` (`"✓".green().bold()`, `.bright_cyan()`); gate quiet output behind `global_flags.should_output()` (`new.rs:36,161`) / `std::env::var("PMCP_QUIET")` (`workbook_server.rs:69`).

### Crate-name validation (path-traversal guard)
**Source:** `cargo-pmcp/src/commands/new.rs:119-140`
**Apply to:** `agent/new.rs` (scaffold name), and validate `package show <path>` is a real OCI layout, and parse `--endpoint` with `url::Url` before use (V5/Input-Validation).

## No Analog Found

None. Every new file maps to a concrete in-repo analog. The only MEDIUM-confidence gap is the **platform capture-API endpoint contract** (out-of-repo, A1/Open-Q2) — but the config/auth/error-handling half of `package capture` has a full analog (`configure::resolver` + `auth_cmd::cache`); only the exact `{api_url}` capture path and payload shape need platform coordination. That is a data/contract gap, not a missing code pattern.

## Metadata

**Analog search scope:** `cargo-pmcp/src/commands/{workbook,new,configure,auth_cmd,connect,auth}`, `cargo-pmcp/src/templates/`, `cargo-pmcp/src/main.rs`, `cargo-pmcp/tests/`, `crates/pmcp-agent/examples/`, `crates/pmcp-team-servers/examples/`, `crates/pmcp-package/src/oci/`
**Files scanned:** ~14 (7 read in full, 7 via targeted grep of public API surface)
**Pattern extraction date:** 2026-07-19
</content>
</invoke>
