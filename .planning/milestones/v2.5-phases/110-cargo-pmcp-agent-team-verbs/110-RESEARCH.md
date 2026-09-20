# Phase 110: cargo-pmcp Agent & Team Verbs - Research

**Researched:** 2026-07-18
**Domain:** Rust CLI (clap) thin-wrapper verbs over already-shipped workspace crates (`pmcp-agent`, `pmcp-team-servers`, `pmcp-package`)
**Confidence:** HIGH (all upstream APIs and cargo-pmcp patterns verified in-repo; nothing depends on external/stale sources)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Ship as **dedicated nested subcommand groups** — `cargo pmcp agent new|dev`, `cargo pmcp team dev`, `cargo pmcp package capture|show` — mirroring the existing `workbook`/`test`/`add` groups in `cargo-pmcp/src/main.rs`. Do NOT overload top-level `new --kind` for agents.
- **D-01a:** `agent new` MAY reuse the existing scaffolding engine (`commands::new` / `templates/`) internally, but is surfaced under the `agent` group, not as a `--kind`. Planner decides share-vs-fork of `commands::new`/templates.
- **D-02:** `team dev` is a **thin CLI over the Phase 109 `TeamRuntime` wiring API** (must NOT re-implement composition). Default: run the doc-review E2E scenario on a **FixedSource** and print a **labeled transcript**. Self-contained, deterministic, offline.
- **D-02a:** `--serve` is an **opt-in** flag exposing team-mcp over HTTP (109 HTTP-first path) so a developer drives the team from their own MCP client.
- **D-02b:** `--llm <endpoint>` swaps FixedSource for a real completion source; absent it, FixedSource is default (zero external services).
- **D-03:** `agent dev` takes `--source openai-compat|sampling|fixed`:
  - `openai-compat` (**default**) → agent loop against OpenAI-compatible endpoint, default `http://localhost:11434/v1` (Ollama), `--endpoint` override. Requires `pmcp-agent` `openai-compat` feature.
  - `sampling` → agent as **sampling-hosted server** (Phase 108 agent-as-server adapter over `ServerCore`, native-only); MCP host provides LLM via `sampling/createMessage`.
  - `fixed` → canned offline/CI mode (FixedSource), no external LLM.
- **D-03a:** Default endpoint assumption is Ollama localhost, NOT auto-detection. If unreachable, fail with actionable message naming `--endpoint`/`--source`.
- **D-04:** `package show` reads and renders a **local `.pmcp` package fully offline** — no platform dependency (uses `pmcp-package`'s own parse/render). Always-works path.
- **D-04a:** `package capture` is a **thin client requiring a configured platform target** — reuse existing `cargo pmcp configure` targets + `cargo pmcp auth` token cache. No target/credentials → fail with actionable guidance (name `configure`/`auth`), never a silent stub or panic.
- **D-04b:** `pmcp-package` pinned at **`"0.1"` (caret)** per CLI-04; a pin tripwire test asserts the dependency version, matching the scaffold-pin tripwire pattern already in cargo-pmcp.
- **D-05:** CLI-01 (`agent new`) ships a generated tripwire test pinning the scaffold's `pmcp-agent` dependency; CLI-04 pins `pmcp-package = "0.1"`. Both reuse the existing cargo-pmcp scaffold-pin tripwire mechanism — mirror it, do not invent a new one.

### Claude's Discretion
- Exact clap struct layout, flag naming beyond the pivotal ones, help text, and how much of `commands::new` scaffolder is shared vs forked.
- cargo-pmcp version bump magnitude (minor for new verbs) and downstream dep version lines — resolve at plan/release time.
- Transcript formatting for the `team dev` demo (labels, coloring) — planner's call.

### Deferred Ideas (OUT OF SCOPE)
- **AgentCore deploy adapter** — deferred follow-on; agents deploy via existing target adapters (agent-as-server is just a server binary).
- **Platform-side capture API / ECR registry / import service** — platform-owned; this phase ships only thin clients.
- **Distributed / multi-process teams** — out of scope; `team dev` is in-process ("small team, one process").
- **`agent dev` auto-detect of local endpoint** — rejected in favor of explicit `--source`/`--endpoint`.
- **Interactive `team dev` REPL** — deferred in favor of the deterministic scripted transcript + `--serve`.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-01 | `cargo pmcp agent new` scaffolds an agent project (AgentPackage manifest + standalone runner) with a version-pin tripwire test against `pmcp-agent` | Reuse `commands::new` scaffolder + a new `templates/agent.rs` emitting `AgentPackage` JSON manifest + `src/main.rs` runner over `AgentEngine`/`OpenAiCompatSource`; mirror the workbook `PMCP_VERSION`/`TOOLKIT_VERSION` drift-guard const+test for `pmcp-agent` (§Standard Stack, §Pattern 4) |
| CLI-02 | `cargo pmcp agent dev` runs an agent locally against an OpenAI-compat endpoint or as a sampling-hosted server | The two modes are fully realized in `pmcp-agent/examples/s50_standalone_vs_sampled.rs` — `agent dev` shells over the same APIs (`OpenAiCompatSource::new`, `AgentEngine::run`, `AgentServer::builder`) (§Pattern 1, §Code Examples) |
| CLI-03 | `cargo pmcp team dev` runs an in-process small team wired from a `TeamPackage` | `pmcp-team-servers/examples/doc_review_team.rs` IS the reference transcript; drive `TeamRuntimeBuilder` + `TeamRuntime` clients (§Pattern 2, §Code Examples) |
| CLI-04 | `cargo pmcp package capture|show` thin clients, `pmcp-package = "0.1"` (caret) + pin tripwire test | `show` = `OciLayout::open` + `unpack_agent/team/server/workflow` (offline). `capture` = reuse `configure::resolver` + `auth_cmd::cache` + reqwest for an authenticated platform call (§Pattern 3) |
</phase_requirements>

## Summary

Phase 110 is a **thin-CLI-over-shipped-crates** phase: it adds four new clap subcommand groups to the `cargo-pmcp` binary. Every piece of runtime behavior it needs already exists and is verified in-repo — the two `agent dev` modes are demonstrated end-to-end in `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs`, the full `team dev` transcript flow is `crates/pmcp-team-servers/examples/doc_review_team.rs`, and `package show`/`capture` compose `pmcp-package`'s `OciLayout`/`unpack_*` API with cargo-pmcp's existing `configure`/`auth_cmd` config plumbing. This phase writes almost no new algorithmic code; the work is CLI surface, argument plumbing, output formatting, dependency wiring, and version-pin tripwire tests.

The single biggest structural change is **dependency wiring**: cargo-pmcp does NOT currently depend on `pmcp-agent`, `pmcp-team-servers`, or `pmcp-package` — this phase adds them (path + version). Design §5 already anticipated this (cargo-pmcp moves after all three in publish order). `team dev` pulls `pmcp-team-servers` (which transitively pulls `pmcp-agent` + `pmcp-package`); `package show/capture` pull `pmcp-package` directly. Feature flags matter: `agent dev` default (`openai-compat`) needs `pmcp-agent/openai-compat`; `team dev --serve` needs `pmcp-team-servers/http`; `team dev --llm` needs `pmcp-team-servers/member-llm`.

**Primary recommendation:** Add `commands::agent`, `commands::team`, `commands::package` modules + three `enum Commands` arms in `main.rs` following the `Workbook` group pattern exactly. Reuse `commands::new`'s `validate_crate_name` + a new `templates/agent.rs` for `agent new`. Shell `agent dev` and `team dev` directly over the two existing example flows. Build `package show` on `OciLayout::open` + media-type-dispatched `unpack_*`, and `package capture` on `configure::resolver::resolve_target` + `auth_cmd::cache::TokenCacheV1`. Mirror the workbook `PMCP_VERSION` const+drift-test for all pins.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `agent new` scaffolding | cargo-pmcp CLI (scaffolder) | `pmcp-package` (AgentPackage manifest shape) | Scaffolding is a CLI concern; the emitted manifest must match `pmcp-package`'s `AgentPackage` struct |
| `agent dev` (openai-compat/fixed) | cargo-pmcp CLI → `pmcp-agent::AgentEngine` | `pmcp-agent` sources | The loop is owned by `pmcp-agent`; the CLI only constructs source+invoker+store and calls `.run()` |
| `agent dev` (sampling-hosted) | cargo-pmcp CLI → `pmcp-agent::AgentServer` | pmcp `ServerCore` + native transport | Agent-as-server adapter is native-only; CLI wires a transport and runs it |
| `team dev` composition | `pmcp-team-servers::TeamRuntime` | `pmcp-agent` (members), `pmcp-package` (`TeamPackage`) | D-02: composition is NOT re-implemented in the CLI; the CLI drives the wiring API |
| `team dev --serve` (HTTP) | `pmcp-team-servers` `http` feature | pmcp `streamable-http` | HTTP-first team-mcp binary path owned by Phase 109 |
| `package show` (render) | cargo-pmcp CLI → `pmcp-package` `OciLayout`/`unpack_*` | — | Fully offline parse/render; no platform, no network |
| `package capture` (upload) | cargo-pmcp CLI (configure/auth) → platform HTTP API | reqwest | Thin client only; the capture service itself is platform-owned |

## Standard Stack

### Core (all already in the workspace — no external discovery needed)
| Crate | Version | Purpose | Why Standard |
|-------|---------|---------|--------------|
| `pmcp-agent` | `0.1` (path `../crates/pmcp-agent`) | `agent new` runner target + `agent dev` loop/sources/adapter | The Phase 108 crate this phase wraps (consume as-is) `[VERIFIED: crates/pmcp-agent/Cargo.toml]` |
| `pmcp-team-servers` | `0.1` (path `../crates/pmcp-team-servers`) | `team dev` in-process runtime + `--serve`/`--llm` features | The Phase 109 crate this phase wraps `[VERIFIED: crates/pmcp-team-servers/Cargo.toml]` |
| `pmcp-package` | `0.1` (caret, path `../crates/pmcp-package`) | `agent new` manifest shape + `package show`/`capture` | CLI-04 mandates caret `"0.1"` `[VERIFIED: crates/pmcp-package/Cargo.toml, CONTEXT D-04b]` |
| `clap` | `4` (features `derive`, `env`) | The subcommand groups | Already the cargo-pmcp CLI framework `[VERIFIED: cargo-pmcp/Cargo.toml]` |
| `reqwest` | `0.13` (rustls, json, multipart) | `package capture` authenticated HTTP upload | Already a cargo-pmcp dep — no new package `[VERIFIED: cargo-pmcp/Cargo.toml]` |
| `colored` | `3` | Transcript / next-steps output styling | Already the cargo-pmcp output convention `[VERIFIED]` |
| `tokio` | `1` (full) | `agent dev`/`team dev` are async | Already a cargo-pmcp dep `[VERIFIED]` |

### Supporting (test-only, already in workspace tree)
| Crate | Version | Purpose | When to Use |
|-------|---------|---------|-------------|
| `assert_cmd` / `predicates` | `2` / `3` | CLI acceptance tests (mirror `tests/cli_acceptance.rs`) | Verifying the new verbs' argument parsing + exit behavior `[VERIFIED: cargo-pmcp dev-deps]` |
| `semver` | `1` | Construct `AgentPackage`/`TeamPackage` `version`/`ComponentRef` in tests | Building manifest fixtures (fields are `semver::Version`/`VersionReq`) `[VERIFIED]` |
| `tempfile` | `3` | Scratch dirs for scaffold + `package show` fixtures | Isolated scaffold/round-trip tests `[VERIFIED]` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Depending on `pmcp-team-servers` for `team dev` | Re-implement composition in the CLI | REJECTED by D-02 — must be a thin CLI over `TeamRuntime`, not a reimplementation |
| A new `templates/agent.rs` | Extend an existing template | Agent scaffold output (AgentPackage JSON manifest + agent-loop runner) is structurally distinct from the SQL/OpenAPI/workbook server templates; a dedicated template is cleaner (planner's D-01a call) |
| Media-type dispatch for `package show` | Try each `unpack_*` in turn | Media-type read from the OCI manifest is deterministic and avoids ambiguous failures (§Pattern 3) |

**Installation (Cargo.toml edits to `cargo-pmcp/Cargo.toml`):**
```toml
[dependencies]
pmcp-agent = { version = "0.1", path = "../crates/pmcp-agent", features = ["openai-compat"] }
pmcp-team-servers = { version = "0.1", path = "../crates/pmcp-team-servers", features = ["runtime", "http", "member-llm"] }
pmcp-package = { version = "0.1", path = "../crates/pmcp-package" }
```
*Feature notes:* `agent dev` default source needs `pmcp-agent/openai-compat`. `team dev` default needs the `runtime` set; `--serve` needs `http`; `--llm` needs `member-llm`. `pmcp-team-servers/http` transitively enables `member-llm` + `pmcp/streamable-http` + `pmcp/http` (verified in its Cargo.toml). cargo-pmcp already enables `pmcp` with `streamable-http`+`oauth`, so no conflict. **Planner must decide** whether to gate `--serve`/`--llm` features always-on or behind a cargo-pmcp feature; simplest is always-on since cargo-pmcp is a native binary with no wasm/size constraint.

**Version verification:** All three crates are `0.1.0` path members of this workspace `[VERIFIED: in-repo Cargo.toml reads 2026-07-18]`. Root `pmcp` is `2.17.0`. No crates.io lookup applies — these resolve via path during dev; the `version` field only matters at publish time (design §5 sequences them before cargo-pmcp).

## Package Legitimacy Audit

> No new **external** third-party packages are introduced by this phase. Every dependency added is an in-repo workspace crate; every external crate needed (`clap`, `reqwest`, `colored`, `tokio`, `serde_json`, `assert_cmd`, `tempfile`, `semver`) is already a declared cargo-pmcp dependency.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `pmcp-agent` | in-repo path crate | Phase 108 | n/a | this repo (`crates/pmcp-agent`) | n/a (workspace) | Approved |
| `pmcp-team-servers` | in-repo path crate | Phase 109 | n/a | this repo (`crates/pmcp-team-servers`) | n/a (workspace) | Approved |
| `pmcp-package` | in-repo path crate (publish=true) | Phase 107 | n/a | this repo (`crates/pmcp-package`) | n/a (workspace) | Approved |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none
*slopcheck not applicable — zero new registry packages. All additions are first-party workspace crates already reviewed in Phases 107–109.*

## Architecture Patterns

### System Architecture Diagram

```
                          cargo pmcp <verb>
                                 │
        ┌────────────────┬───────┴────────┬─────────────────┐
        ▼                ▼                ▼                  ▼
   agent new        agent dev          team dev       package show|capture
        │                │                │                  │
        │        ┌───────┴──────┐         │           ┌──────┴───────┐
        ▼        ▼              ▼         ▼           ▼              ▼
  templates/  --source=      --source=  TeamRuntime  show:         capture:
  agent.rs    openai-compat  sampling   Builder      OciLayout     configure::
  (reuse      /fixed         (hosted)   .build(pkg)  ::open +      resolver +
  validate_   │              │          │            unpack_* +    auth_cmd::
  crate_name) ▼              ▼          ▼            media-type    cache +
        │  AgentEngine   AgentServer  TeamRuntime    dispatch      reqwest POST
        │  ::new(...)    ::builder(). .team_fs_      (OFFLINE)     to platform
        │  .run()        build().run  client()/etc          │     capture API
        ▼     │  ▲          │           │            render manifest    │
  AgentPackage │  │          │           ▼                  │      (needs target
  JSON manifest│  │          │      labeled transcript      │       + token or
  + src/main.rs│  │          │      (doc-review flow)        │       actionable
  + tripwire   ▼  │          ▼                               ▼       error)
  test    OpenAiCompat  in-process/native   ─── all output via colored println ───
          Source        transport + host
          (Ollama)      sampling
```

### Recommended Project Structure (additions to `cargo-pmcp/src/`)
```
commands/
├── agent/
│   ├── mod.rs        # AgentCommand enum (New | Dev) + execute dispatch
│   ├── new.rs        # thin: validate_crate_name + templates::agent::generate
│   └── dev.rs        # --source resolution → AgentEngine or AgentServer run
├── team/
│   ├── mod.rs        # TeamCommand enum (Dev) + execute dispatch
│   └── dev.rs        # TeamRuntimeBuilder drive + labeled transcript; --serve/--llm
├── package/
│   ├── mod.rs        # PackageCommand enum (Capture | Show) + execute dispatch
│   ├── show.rs       # OciLayout::open + media-type dispatch + render
│   └── capture.rs    # configure+auth reuse + reqwest upload
templates/
└── agent.rs          # AgentPackage manifest + runner + PMCP_AGENT_VERSION const/tripwire
```

### Pattern 1: `agent dev` — one loop, two sources (verified end-to-end)
**What:** Resolve `--source` to a `CompletionSource` (openai-compat/fixed) driven directly by `AgentEngine`, OR to a sampling-hosted `AgentServer`.
**When to use:** CLI-02.
**Source:** `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs` (the canonical two-mode demo).
```rust
// Source: crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs (verified in-repo)
// --- openai-compat / fixed path: engine over a source ---
use pmcp_agent::{AgentEngine, InMemoryStore, ResolvedAgentConfig, RunOutcome};
use pmcp_agent::sources::{OpenAiCompatSource, SecretString}; // feature = "openai-compat"

let config = ResolvedAgentConfig::new(
    "You are a concise assistant.", "llama3.2", 100_000, 5,
);
let source = OpenAiCompatSource::new(
    "http://localhost:11434/v1", "llama3.2", SecretString::new("ollama"),
)?; // if endpoint unreachable, the first .run() surfaces a transport error → map to
    // an actionable message naming --endpoint/--source (D-03a)
let engine = AgentEngine::new(source, invoker, InMemoryStore::new(), config);
let outcome: RunOutcome = engine.run("agent-dev-run").await;

// --- sampling-hosted path: expose as a server, host provides the LLM ---
use pmcp_agent::{AgentServer, SamplingSourceFactory, CompletionSourceFactory};
let factory: Arc<dyn CompletionSourceFactory> = Arc::new(SamplingSourceFactory::new());
let agent = AgentServer::builder(package, config, factory, invoker, store).build()?;
// agent.run(transport).await — native-only; run over stdio/HTTP transport
```
**Note:** `fixed` source = inject a scripted/end-turn `CompletionSource` via `FixedSourceFactory` (offline/CI). The invoker for a real run is `ClientToolInvoker` (`pmcp-agent::invoker`); for a minimal demo a no-op invoker suffices — planner decides how much tool wiring `agent dev` exposes vs a bare loop.

### Pattern 2: `team dev` — drive the in-process TeamRuntime (verified end-to-end)
**What:** Build a `TeamRuntime` from a `TeamPackage` over in-memory transports with a `FixedSource` override, then call the per-server clients in the doc-review order and print a labeled transcript.
**When to use:** CLI-03 default (no `--serve`).
**Source:** `crates/pmcp-team-servers/examples/doc_review_team.rs` + `tests/small_team.rs`.
```rust
// Source: crates/pmcp-team-servers/examples/doc_review_team.rs (verified in-repo)
use pmcp_team_servers::compose::resolver::LocalDirPackageResolver;
use pmcp_team_servers::compose::wiring::TeamRuntimeBuilder;
use pmcp_agent::{FixedSourceFactory, ProgrammaticBuilder}; // SlotResolver stub

let rt = TeamRuntimeBuilder::new(resolver, stub_slot_resolver())
    .with_completion_override(fixed_override())   // offline default (D-02b)
    .with_data_root(data_dir.path())
    .build(&team_package)                          // &TeamPackage
    .await?;

let team_fs  = rt.team_fs_client().expect("team-fs attached (opt-in)");
let approval = rt.approval_client().expect("approval-mcp attached (1 human)");
let mem      = rt.mem_client().expect("mem-mcp attached (opt-in)");
let team_mcp = rt.team_mcp_client().expect("team-mcp attached (members)");
// then: fs__write → fs__sync_to_review → team_approval__ask_<role> →
// resolve_approval → fs__read → mem__add → team_mcp__<member> dispatch
// (each returns a CallToolResult; print a labeled line per step = the transcript)
```
**`--serve` (D-02a):** build `pmcp-team-servers` with the `http` feature and take the HTTP-first team-mcp binary path (109 D-04's "expose externally" route) instead of driving the flow in-process. **`--llm` (D-02b):** replace `with_completion_override(fixed_override())` with a real `OpenAiCompatSource`-backed factory (needs `member-llm` feature).

### Pattern 3: `package show`/`capture` — OCI layout + config reuse
**What:** `show` opens a local OCI image-layout package, reads the manifest media type to pick the kind, unpacks the typed manifest, and renders it. `capture` resolves the active platform target + cached token and POSTs.
**When to use:** CLI-04.
**Source:** `crates/pmcp-package/src/oci/{layout.rs,unpack.rs,media_types.rs}` + `cargo-pmcp/src/commands/{configure,auth_cmd}`.
```rust
// Source: crates/pmcp-package/src/oci (verified in-repo)
use pmcp_package::oci::{OciLayout, unpack_agent, unpack_team, unpack_server, unpack_workflow};
// A `.pmcp` package is an OCI image-layout DIRECTORY (blobs/sha256/ + index.json).
let layout = OciLayout::open(path);          // infallible open
let index = layout.read_index()?;            // ImageIndex → manifest descriptor
// Inspect the manifest's config/layer media types (MT_AGENT_CONFIG /
// MT_TEAM_CONFIG / MT_SERVER_ENVELOPE / MT_WORKFLOW_MANIFEST) to pick the kind:
let pkg = unpack_agent(&layout)?;            // -> AgentPackage (render its fields)
// unpack_team -> TeamPackage; unpack_server -> (ServerPackage, Vec<u8>); unpack_workflow -> WorkflowManifest
```
```rust
// package capture: reuse existing config plumbing (D-04a), do NOT invent new config
use cargo_pmcp::commands::configure::resolver::{resolve_active_target_name, resolve_target};
use cargo_pmcp::commands::auth_cmd::cache::{TokenCacheV1, default_multi_cache_path, normalize_cache_key};
// 1. resolve target (api_url) → if none, bail! naming `cargo pmcp configure add <name>`
// 2. read token cache keyed by the target's api_url → if missing, bail! naming `cargo pmcp auth login <url>`
// 3. reqwest POST the packed package to the platform capture API with Bearer token
//    (mirror the authenticated-call pattern in deployment/targets/pmcp_run/deploy.rs)
```
**Key detail:** the "capture API" endpoint shape is **platform-owned and not specified in-repo** — see Open Questions. `capture` must degrade to an actionable error when unconfigured, NEVER a panic (D-04a).

### Pattern 4: Version-pin tripwire (mirror the workbook drift guard)
**What:** A hardcoded pin constant in the template + a unit test asserting it equals the workspace crate's `[package] version`, so the emitted scaffold's pin cannot silently drift from the released crate.
**Source:** `cargo-pmcp/src/templates/workbook_server.rs` (`PMCP_VERSION`/`TOOLKIT_VERSION` + `emitted_*_matches_workspace_pin` tests).
```rust
// Source: cargo-pmcp/src/templates/workbook_server.rs (verified in-repo)
const PMCP_AGENT_VERSION: &str = "0.1.0"; // pin the agent scaffold emits
#[test]
fn emitted_agent_version_matches_workspace_pin() {
    const AGENT_CARGO_TOML: &str = include_str!("../../../crates/pmcp-agent/Cargo.toml");
    let parsed: toml::Value = toml::from_str(AGENT_CARGO_TOML).unwrap();
    let v = parsed["package"]["version"].as_str().unwrap();
    assert_eq!(PMCP_AGENT_VERSION, v, "agent scaffold pin drifted — bump PMCP_AGENT_VERSION");
}
```
**Two distinct tripwires required (D-05):**
1. **CLI-01:** `agent new`'s *generated* project should contain a test asserting its `Cargo.toml` pins `pmcp-agent` (a `tests/pin.rs` in the scaffold) **and** the template carries the internal drift-guard above. See Open Question Q1 on which "level" the requirement means.
2. **CLI-04:** a cargo-pmcp-internal test asserting `cargo-pmcp/Cargo.toml`'s `pmcp-package` dependency line is caret `"0.1"` (parse cargo-pmcp's own Cargo.toml, assert the `pmcp-package` req string == `"0.1"`).

### Anti-Patterns to Avoid
- **Re-implementing team composition in the CLI** — D-02 forbids it; drive `TeamRuntime`.
- **Overloading `new --kind agent`** — D-01 forbids it; use a dedicated `agent` group.
- **Inventing new config/auth for `package capture`** — D-04a forbids it; reuse `configure`/`auth_cmd`.
- **Panicking or silently stubbing when `capture` is unconfigured** — D-04a: actionable error only.
- **`=0.1.0` exact pin for `pmcp-package`** — explicitly Out of Scope in REQUIREMENTS; caret `"0.1"`.
- **Hardcoding a pin without a drift test** — the whole point of the tripwire (Toyota Way / the workbook precedent).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Agent decision loop | A tool-call/iteration loop in the CLI | `pmcp_agent::AgentEngine` | Pure, replay-safe, property-tested (AGNT-02/03) |
| Team composition/wiring | Manual server spin-up + transport plumbing | `pmcp_team_servers::TeamRuntimeBuilder` | D-02; composition-derived attachment already solved in 109 |
| OCI package parse/render | Custom `.pmcp` reader | `pmcp_package::OciLayout` + `unpack_*` | Canonical digest + media-type layout already implemented (Phase 107) |
| Platform target resolution | New TOML config format | `configure::resolver::resolve_target` | D-04a; `~/.pmcp/config.toml` + active-target already the convention |
| OAuth token retrieval | New token store | `auth_cmd::cache::TokenCacheV1` | D-04a; atomic multi-server token cache already exists |
| Crate-name validation | Ad-hoc string checks | `commands::new::validate_crate_name` | Path-traversal + Cargo-name guard already hardened (T-86-03-02) |
| Pin-drift detection | Manual version bookkeeping | The workbook `const + include_str! + assert_eq!` test | Proven mechanism; D-05 mandates reuse |

**Key insight:** This phase's correctness comes almost entirely from *not* writing new logic — every hard problem (loop purity, composition, digest stability, auth) is already solved and tested in Phases 107–109. The failure mode is duplicating that logic in the CLI instead of shelling over it.

## Common Pitfalls

### Pitfall 1: Feature-flag omission breaks `agent dev`/`team dev` at runtime
**What goes wrong:** `agent dev --source openai-compat` fails to compile or the source is missing because `pmcp-agent/openai-compat` wasn't enabled; `team dev --serve` has no HTTP path because `pmcp-team-servers/http` wasn't enabled.
**Why it happens:** Both crates keep HTTP/LLM sources behind non-default features (wasm-cleanliness in their own trees).
**How to avoid:** Enable `pmcp-agent` `openai-compat` and `pmcp-team-servers` `runtime,http,member-llm` in cargo-pmcp's dep lines (see Installation). Add a CLI acceptance test that exercises each `--source`/`--serve`/`--llm` path.
**Warning signs:** `error[E0432]: unresolved import pmcp_agent::sources::OpenAiCompatSource`, or a `--serve` arm that only errors.

### Pitfall 2: Publish-order / version-line drift
**What goes wrong:** cargo-pmcp fails to publish because `pmcp-agent`/`pmcp-team-servers`/`pmcp-package` aren't published yet, or a version-req line is stale.
**Why it happens:** These are new publish-order entries (design §5); cargo-pmcp now depends on three 0.x crates.
**How to avoid:** Use `version = "0.1"` + `path` on the dep lines (path wins locally, version applies at publish). Note the ordering in the plan; the release workflow skips already-published crates.
**Warning signs:** `cargo publish` "no matching package named pmcp-agent found".

### Pitfall 3: `package show` kind mis-detection
**What goes wrong:** Calling `unpack_agent` on a team package (or vice versa) yields a confusing parse error.
**Why it happens:** All four kinds share the OCI layout; only the manifest media type distinguishes them (`MT_AGENT_CONFIG`/`MT_TEAM_CONFIG`/`MT_SERVER_ENVELOPE`/`MT_WORKFLOW_MANIFEST`).
**How to avoid:** Read the manifest/config descriptor's media type from `read_index`/`read_manifest` first, then dispatch to the matching `unpack_*`. Provide a clear "unknown package kind" error otherwise.
**Warning signs:** `PackageError` on a valid package the user knows is a team.

### Pitfall 4: `agent dev` default endpoint hangs instead of failing fast
**What goes wrong:** With no Ollama running on `localhost:11434`, the run hangs or emits a raw connection error.
**Why it happens:** Default source is openai-compat→Ollama (D-03a), and the transport error surfaces on first completion.
**How to avoid:** Catch the `CompletionError::Transport` from the first `.run()`/completion and re-emit an actionable message naming `--endpoint`/`--source fixed` (D-03a explicitly requires this). Consider a fast preflight (or short connect timeout).
**Warning signs:** `agent dev` appears to hang with no output on a machine without Ollama.

### Pitfall 5: `make quality-gate` clippy pedantic/nursery on the new CLI code
**What goes wrong:** CI fails on lints that a bare `cargo clippy` misses (CLAUDE.md: CI uses `make lint` with pedantic+nursery on the root `pmcp` crate). **Note:** cargo-pmcp is NOT clippy-gated the same way (per MEMORY `project_rust195_clippy_gate_debt`), but PMAT cognitive-complexity ≤25 and `make quality-gate` (fmt/clippy/build/test) still apply.
**How to avoid:** Run `make quality-gate` before commit; keep each new command handler under cog-complexity 25 (split `--source`/`--serve` dispatch into small helpers, mirroring `workbook/mod.rs`).
**Warning signs:** PMAT `quality-gate` job flags a new `execute` fn > cog 25.

## Code Examples

### `enum Commands` arm + dispatch (mirror the Workbook group)
```rust
// Source: cargo-pmcp/src/main.rs (verified in-repo) — add three arms:
#[derive(Subcommand)]
enum Commands {
    // ... existing ...
    /// Scaffold and run agents (AgentPackage-backed)
    Agent { #[command(subcommand)] command: commands::agent::AgentCommand },
    /// Run an in-process small team from a TeamPackage
    Team { #[command(subcommand)] command: commands::team::TeamCommand },
    /// Capture/show portable .pmcp packages
    Package { #[command(subcommand)] command: commands::package::PackageCommand },
}
// dispatch (around main.rs:555):
Commands::Agent { command } => command.execute(global_flags).await, // async: agent dev
Commands::Team { command }  => command.execute(global_flags).await, // async: team dev
Commands::Package { command } => command.execute(global_flags).await,
```
**Async note:** existing groups (`Workbook`) are sync `execute(&GlobalFlags)`. `agent dev`/`team dev`/`package capture` are async. Follow the `Loadtest`/`Auth`/`Deploy` groups (which are async) for the `#[tokio::main]`/`block_on` wiring already present in `main.rs`.

### AgentPackage manifest fields the `agent new` scaffold must emit
```rust
// Source: crates/pmcp-package/src/package/agent.rs + example fixtures (verified)
AgentPackage {
    name, version: semver::Version, instructions,
    llm: ConfigSlot { slot: SlotType::LlmProvider { name, tested_value } },
    max_tokens: i64, max_iterations: i64,
    connectors: vec![], tool_selection: None, input_schema: None,
    output_schema: None, importance: None, finalizer_role: None, budget_defaults: vec![],
}
```

## Runtime State Inventory

> Not applicable — this is an additive greenfield CLI phase (new subcommand groups + new dependency lines). No rename/refactor/migration of existing stored data, service config, OS-registered state, secrets, or build artifacts. Verified: the phase touches only new `commands/{agent,team,package}` modules, a new `templates/agent.rs`, three `Cargo.toml` dep lines, and three `enum Commands` arms.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Servers-only cargo-pmcp on-ramp (`new --kind sql/openapi/workbook-server`, `dev`) | Adds agents + teams as first-class verbs | Phase 110 (this) | cargo-pmcp becomes the agent/team on-ramp, matching its server story |
| Raw-JSON-RPC team dispatch (platform bypass) | `team_mcp__<member>` tools + `TeamRuntime` composition | Phase 109 | `team dev` demonstrates the sanctioned migration template |
| Hand-rolled agent loops | `pmcp-agent::AgentEngine` (pure, replay-safe) | Phase 108 | `agent dev`/`agent new` target one open loop |

**Deprecated/outdated:** none relevant — all wrapped crates are current (`0.1.0`) as of this milestone.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The platform "capture API" endpoint/verb/payload shape is not defined in-repo and `package capture` must target a platform-owned HTTP endpoint | Pattern 3, Open Q2 | Without the endpoint contract, `capture` can only ship the config/auth wiring + actionable errors + a documented request shape; a wrong guess would produce a non-functional call. Planner should scope `capture` to "resolve+authenticate+POST to configured api_url path, error cleanly when unconfigured" and flag the exact endpoint as needing platform input. |
| A2 | A `.pmcp` "package file" is an OCI image-layout **directory** (blobs/sha256 + index.json), consumed via `OciLayout::open` | Pattern 3 | If packages are distributed as a single tar/archive file, `show` needs an unarchive step first. `pmcp-package` exposes only `OciLayout` (dir-based) + `pack_*`/`unpack_*`; no archive reader was found. Confirm the on-disk shape `agent new`/capture produce. |
| A3 | Always-on `http`+`member-llm` features on the `pmcp-team-servers` dep are acceptable (no cargo-pmcp feature gating needed) | Standard Stack | If binary size / build time matters, planner may gate them behind a cargo-pmcp feature; low risk for a native dev CLI. |
| A4 | CLI-01's "generated tripwire test against pmcp-agent" means a test **inside the scaffolded project** (plus an internal template drift-guard), not only the internal drift-guard | Pattern 4, Open Q1 | If it means only the internal guard, the scaffold need not emit a `tests/` file — less work. Surfaced as Q1 for discuss/plan confirmation. |

## Open Questions (RESOLVED)

1. **Does CLI-01's tripwire live in the generated project, in cargo-pmcp, or both?**
   - What we know: existing templates carry only an *internal* drift-guard (const + `include_str!` + `assert_eq!` against the workspace crate version); they do NOT emit a `tests/` file into the scaffold. CLI-01/D-05 wording says "a generated tripwire test pinning the scaffold's `pmcp-agent` dependency."
   - What's unclear: whether "generated" means emitted-into-the-scaffold or generated-by-the-template-author (i.e. the internal guard).
   - Recommendation: ship BOTH (safest, satisfies the literal wording) — an emitted `tests/pin.rs` in the agent scaffold asserting its `pmcp-agent` req, plus the internal drift-guard so the hardcoded pin can't drift from the released `pmcp-agent`. Confirm at plan time.
   - **RESOLVED:** ship BOTH tripwire levels — an emitted in-scaffold `tests/pin.rs` AND the internal `emitted_agent_version_matches_workspace_pin` template drift-guard. Owned by plan **110-02 Task 2** (emitter) + Task 3 (wiring).

2. **What is the platform capture API contract (endpoint path, method, payload, auth header)?**
   - What we know: it's platform-owned (design §"Package" ownership table; REQUIREMENTS Out of Scope); cargo-pmcp ships only a thin client reusing `configure` (api_url) + `auth_cmd` (Bearer token). The authenticated-call precedent is `deployment/targets/pmcp_run/deploy.rs`.
   - What's unclear: the exact route/body the capture service expects.
   - Recommendation: implement `capture` to resolve target+token and POST the packed package to a `{api_url}/…capture` path; make the path a constant/config the planner can confirm; ensure the unconfigured path errors with actionable guidance. Flag the endpoint as a platform-coordination item (does not block the CLI/config/auth/error-handling work).
   - **RESOLVED:** `capture` resolves target+token (reusing `configure`/`auth`) and POSTs to a named `CAPTURE_PATH` constant, with actionable errors when unconfigured; the exact platform endpoint is flagged as a platform-coordination item (threat T-110-05-05, disposition *accept*). Owned by plan **110-05 Task 3**.

3. **How much tool wiring does `agent dev` expose?**
   - What we know: `AgentEngine` needs a `ToolInvoker`; `ClientToolInvoker` connects to real MCP servers, but a demo run can use a no-op/empty invoker.
   - Recommendation: for the first pass, `agent dev` runs the loop with a minimal/no-op invoker (or connectors from the AgentPackage if present); richer tool-server attachment can be a discretionary enhancement. Planner's call.
   - **RESOLVED:** first pass uses a minimal/no-op invoker (records + echoes ok) against the public `ToolInvoker` trait; richer attachment deferred. Owned by plan **110-03 Task 2**.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Ollama (`localhost:11434`) | `agent dev --source openai-compat` (default) | ✗ (not probed; user-local) | — | `--source fixed` (offline) or `--source sampling` |
| Configured platform target (`~/.pmcp/config.toml`) | `package capture` | conditional | — | actionable error naming `cargo pmcp configure add` |
| Cached OAuth token | `package capture` | conditional | — | actionable error naming `cargo pmcp auth login` |
| Rust toolchain / cargo | build + all verbs | ✓ (workspace builds) | stable (CI: dtolnay stable) | — |

**Missing dependencies with no fallback:** none — every verb has an offline/actionable-error path (`agent dev` → `fixed`; `team dev` default is offline FixedSource; `package show` is fully offline; `package capture` errors cleanly when unconfigured).
**Missing dependencies with fallback:** Ollama (→ `fixed`/`sampling`); platform target/token (→ actionable error, by design D-04a).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `#[tokio::test]` + `assert_cmd`/`predicates` for CLI acceptance |
| Config file | none (cargo test); `cargo-pmcp/tests/` holds integration tests |
| Quick run command | `cargo test -p cargo-pmcp --lib` (template/unit + tripwire tests) |
| Full suite command | `make quality-gate` (fmt + clippy + build + test + audit, matches CI) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLI-01 | `agent new` emits AgentPackage manifest + runner + tripwire | integration (scaffold) | `cargo test -p cargo-pmcp --test scaffold_agent` | ❌ Wave 0 |
| CLI-01 | template pin doesn't drift from `pmcp-agent` version | unit | `cargo test -p cargo-pmcp --lib emitted_agent_version_matches_workspace_pin` | ❌ Wave 0 |
| CLI-02 | `agent dev` resolves `--source` and runs the loop (fixed mode, offline) | integration | `cargo test -p cargo-pmcp --test agent_dev` | ❌ Wave 0 |
| CLI-03 | `team dev` drives the doc-review transcript offline | integration | `cargo test -p cargo-pmcp --test team_dev` | ❌ Wave 0 |
| CLI-04 | `package show` renders a local package offline | integration | `cargo test -p cargo-pmcp --test package_show` | ❌ Wave 0 |
| CLI-04 | cargo-pmcp pins `pmcp-package = "0.1"` (caret) | unit | `cargo test -p cargo-pmcp --lib pmcp_package_pin_is_caret_0_1` | ❌ Wave 0 |
| CLI-04 | `package capture` errors actionably when unconfigured | integration | `cargo test -p cargo-pmcp --test package_capture` | ❌ Wave 0 |

**Per CLAUDE.md ALWAYS requirements:** each new verb also needs a working `cargo run` example path (the verbs ARE runnable demos), and property/fuzz coverage where a pure function exists (e.g. `package show` kind-detection, transcript formatting). The loop/composition/digest logic is already property/fuzz-tested in the wrapped crates.

### Sampling Rate
- **Per task commit:** `cargo test -p cargo-pmcp --lib`
- **Per wave merge:** `cargo test -p cargo-pmcp`
- **Phase gate:** `make quality-gate` green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `cargo-pmcp/tests/scaffold_agent.rs` — CLI-01 (mirror `scaffold_sql_server.rs`; may reuse `tests/support/scaffold_patch.rs`)
- [ ] `cargo-pmcp/tests/agent_dev.rs` — CLI-02 (fixed-source offline run)
- [ ] `cargo-pmcp/tests/team_dev.rs` — CLI-03 (offline transcript assertion)
- [ ] `cargo-pmcp/tests/package_show.rs` + a small `.pmcp` fixture — CLI-04
- [ ] `cargo-pmcp/tests/package_capture.rs` — CLI-04 (unconfigured → actionable error)
- [ ] template unit tests for the two pin tripwires (in `templates/agent.rs` + a cargo-pmcp dep-line test)

## Security Domain

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | `package capture` reuses `auth_cmd::cache` Bearer tokens — do NOT log tokens; reuse `secrecy`/redaction already in the tree |
| V3 Session Management | no | CLI is stateless per invocation |
| V4 Access Control | no | no multi-user surface |
| V5 Input Validation | yes | `agent new` name → reuse `commands::new::validate_crate_name` (path-traversal + Cargo-name guard); `package show <path>` → validate the path is a real OCI layout before unpack; `--endpoint` URL → parse/validate before use |
| V6 Cryptography | no (delegated) | digest/canonicalization is `pmcp-package`'s concern (sha256); never re-implement |

### Known Threat Patterns for this stack (Rust CLI + local files + one authenticated upload)
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal via scaffold name / package path | Tampering | `validate_crate_name` (reuse); canonicalize + bound `package show` path |
| Token leakage in logs/errors | Information disclosure | Reuse `secrecy`/`RedactedSecret`; never `println!` a token; the agent `SecretString` already redacts |
| Malicious `.pmcp` package (digest mismatch) | Tampering | `pmcp-package` verifies canonical digest on unpack — surface verification failures, don't bypass |
| Unvalidated `--endpoint` (SSRF-ish local dev) | Tampering/EoP | Parse with `url::Url`; it's a dev-only source, but validate scheme/host and fail fast (D-03a) |

## Sources

### Primary (HIGH confidence — verified in-repo this session)
- `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs` — the two `agent dev` modes end-to-end
- `crates/pmcp-agent/src/{lib.rs,adapter/factory.rs,sources/openai_compat.rs}` — public API for `agent dev`/`agent new`
- `crates/pmcp-team-servers/examples/doc_review_team.rs` + `tests/small_team.rs` — the `team dev` transcript flow and `TeamRuntime` API
- `crates/pmcp-team-servers/{Cargo.toml,src/compose/wiring.rs}` — feature flags + `TeamRuntimeBuilder`/`TeamRuntime`
- `crates/pmcp-package/src/{lib.rs,oci/{layout.rs,unpack.rs,pack.rs,media_types.rs},package/agent.rs}` — `package show`/`capture` building blocks
- `cargo-pmcp/src/main.rs`, `src/commands/{mod.rs,new.rs,dev.rs,workbook/mod.rs,configure/,auth_cmd/}` — the patterns to mirror
- `cargo-pmcp/src/templates/workbook_server.rs` — the scaffold-pin tripwire mechanism (D-05)
- `cargo-pmcp/tests/scaffold_sql_server.rs` — the scaffold integration-test pattern
- `.planning/phases/110-cargo-pmcp-agent-team-verbs/110-CONTEXT.md`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `docs/design/agents-teams-sdk-extraction-plan.md` §Phase E/§5

### Secondary (MEDIUM)
- `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs` — authenticated platform-call precedent for `package capture`

### Tertiary (LOW)
- none — no external/unverified sources were needed (all APIs are first-party and in-repo)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every dependency is an in-repo workspace crate with verified APIs; zero external discovery.
- Architecture: HIGH — `agent dev` and `team dev` are already demonstrated by shipped examples this phase shells over.
- Pitfalls: HIGH — feature-flag/publish-order/kind-detection risks derived directly from the wrapped crates' Cargo.toml and API.
- `package capture` endpoint: MEDIUM-LOW — the platform API contract is out-of-repo (A1/Q2); the config/auth/error-handling half is HIGH.

**Research date:** 2026-07-18
**Valid until:** 2026-08-17 (stable — first-party crates; only the platform capture-API contract may firm up sooner via platform coordination)
