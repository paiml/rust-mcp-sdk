# Phase 109: Team Reference Servers - Research

**Researched:** 2026-07-18
**Domain:** Rust MCP SDK internal crate — reference server implementations, backend-trait seams, in-process agent-team wiring, wire-level conformance harness
**Confidence:** HIGH (all findings verified against in-repo code, contracts, and Phase 108 artifacts; only `pmat comply` CI-wiring mechanics are LOW)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Crate shape & dev-run surface**
- **D-01:** In-process small-team composition ships HERE as a **library wiring API** (all four servers + member agents in one tokio runtime, wired from a `TeamPackage`). Phase 110's `cargo pmcp team dev` is a thin CLI over it. "Small team, one process" is proven by an integration test in this crate.
- **D-02:** Four dev binaries are **HTTP-first**: each binds an HTTP port by default (via pmcp streamable HTTP), stdio available as a fallback flag.
- **D-03:** Binaries configured from a **`TeamPackage` file** (`--package <file>`) as primary config — roster, members, per-server settings all from the same definition Phase 110 and the platform use. Flags/env only override port + data dir.
- **D-04:** The in-process wiring API connects servers and members over **in-memory transports** (no sockets; deterministic tests; CI-sandbox safe). HTTP-first binaries remain the way to expose any server externally.

**Composition-derived wiring**
- **D-05:** **Attachment is derived from team composition.** team-mcp attaches iff `members.len() >= 2`; approval-mcp attaches iff `human_roles` non-empty (channel initiator implicit, never counted). `TeamPackage.built_in_servers` demoted to **opt-in extras**. Team-of-one, zero humans → just the AgentServer, zero wiring config.
- **D-06:** **team-fs and mem-mcp are both fully opt-in** (explicit `built_in_servers` entries). Only team-mcp and approval-mcp are derived. (team-fs does NOT auto-ride with approval-mcp.)
- **D-07:** The derivation rule is a **pure, exported function in `pmcp-team-servers`** (`derive_attachment(&TeamPackage) -> AttachmentSet` + composition-snapshot type), property/unit-tested: N=1,M=0 → agent only; N≥2 → +team-mcp; M≥1 → +approval-mcp; opt-ins honored. Snapshot-at-entry is the documented contract.

**Dev-backend fidelity**
- **D-08:** `fs__get_download_url` on the local-directory dev backend returns a **`file://` URI** to the file's real path. `TeamFsBackend` trait leaves URL semantics to each backend.
- **D-09:** `fs__sync_to_review` / `fs__sync_from_review` get **real local semantics via a sibling `review/` directory**: sync_to_review copies out for a human to edit, sync_from_review copies edits back.
- **D-10:** **Console (dev) approval channel** prints ask (question, options, approval id) to the server console; **resolution ALWAYS via the `resolve_approval` tool** from any connected client. One resolution path for both channels, no TTY dependency, deterministic in CI. No stdin prompting.
- **D-11:** **Webhook (CI) approval channel** is a **notify-only outgoing POST** (ask payload + approval id) to a configured URL; resolution still via `resolve_approval`. Optional shared-secret header; no HMAC machinery.
- **D-12:** Approval ask/resolve records carry an **optional subject reference** (`subject_task_id`/`subject_ref`) stored on the record and echoed by `get_approval`/`resolve_approval`. Contract YAML revs additively to include it.

**team-mcp member wiring**
- **D-13:** team-mcp reaches members as **in-process Phase 108 `AgentServer` instances over in-memory MCP** — a `pmcp::Client` per member, full MCP hop. Exercises the real `ToolOutput::Result` + top-level `related_task` `_meta` path (the TEAM-05 migration template).
- **D-14:** Depth + ancestor-chain guard state travels as **namespaced `_meta` fields on `tools/call`**; the HTTP binary maps the `x-pmcp-team-depth` header into that `_meta` at the edge.
- **D-15:** Member agents get their LLM via **`CompletionSourceFactory` resolved from the member's `AgentPackage` llm `ConfigSlot`** through `SlotResolver`: OpenAI-compat (Ollama) for standalone dev, Anthropic if configured. No outer sampling host required.
- **D-16:** Phase-goal E2E scenario is a **doc-review flow** through all four servers, on FixedSource for CI determinism; real LLM optional.

**Conformance harness**
- **D-17:** Ship an **exportable conformance harness** (module/feature): fixture-driven runner (any server impl + fixture dir), used by this repo AND importable by the platform as a dev-dependency. Fixtures canonical in `contracts/team-servers/fixtures/`.
- **D-18:** This phase **authors the `binding.yaml` and wires `pmat comply check`** for team-servers-v1.
- **D-19:** Runner drives servers through a **real `pmcp::Client` over the in-memory transport**: initialize → `tools/list` (exact set + schema equality) → `tools/call` per fixture.
- **D-20:** Fixture coverage: **every tool + every guard** — ≥1 success dispatch per advertised tool, an error fixture per contract error path (unknown member, malformed/excessive depth, self-call, ancestor-cycle, invalid args), and exact `tools/list` surface fixtures for all four servers.

### Claude's Discretion
- Module layout, feature-flag names, binary names, default ports, CLI flag spelling
- BM25/keyword scoring internals for the `TeamMemoryBackend` dev impl
- Exact namespaced `_meta` key names for depth/ancestry (follow existing pmcp `_meta` conventions)
- Composition-snapshot type shape and the `AttachmentSet` API
- Approval task lifecycle details on the in-memory `TaskStore` (reuse `with_task_store()` infrastructure)
- Fixture file layout/naming for expanded coverage; how fixtures embed (include_dir vs path)
- Contract YAML rev mechanics for the additive subject field + `_meta` depth documentation

### Deferred Ideas (OUT OF SCOPE)
- Sampling passthrough up the chain (member sampling proxied through team-mcp to outer client's LLM)
- HTTP download route with expiring tokens for team-fs dev backend
- Inbound webhook callback endpoint for approval resolution
- Nested-team demo (guards implemented, no nested example)
- team-fs auto-attach with approval-mcp
- Traces-redesign platform items (human-turn span kinds, capture policy, provenance, billing)
- Per-target deploy demos (Phase 110/111)
- mem0-rust / any embedder dependency; distributed/multi-process teams
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TEAM-01 | `crates/pmcp-team-servers` exists with per-server feature flags and runnable dev binaries for all four servers | Standard Stack (new-crate scaffold), Architecture Pattern 1 (crate + feature-gated `[[bin]]`), verified against `pmcp-sql-server`/`pmcp-tasks` precedent |
| TEAM-02 | team-fs serves `fs__*` over a `TeamFsBackend` trait with a local-directory dev backend | Pattern 2 (backend-trait seam), `TaskStore` precedent, D-08/D-09 semantics; `tempfile` for tests |
| TEAM-03 | approval-mcp serves the approval contract over an in-memory `TaskStore` with console + webhook channels | Pattern 3 (approval on `InMemoryTaskStore` + `with_task_store`), D-10/D-11/D-12; `reqwest` (feature-gated) for webhook |
| TEAM-04 | mem-mcp serves `mem__*` over a `TeamMemoryBackend` trait with a keyword/BM25 in-memory dev backend (no embedder) | Pattern 2 + Don't-Hand-Roll note on BM25 (hand-roll vs `bm25` crate) |
| TEAM-05 | team-mcp composes agent-as-server members as per-member tools returning `ToolOutput::Result` with top-level `related_task` `_meta` | Pattern 4 (member dispatch via `pmcp::Client` over `DuplexTransport` → `ToolOutput::Result`), D-13/D-14; **critical**: task-augmented dispatch required to surface `related_task` (see Pitfall 1) |
| TEAM-06 | Conformance tests prove each reference server's tool surface matches PKG-03 contracts | Pattern 5 (wire-level runner over `pmcp::Client`), existing `tests/team_contracts_conformance.rs` structural base, D-17/D-19/D-20 |
</phase_requirements>

## Summary

This is an **internal Rust SDK phase**: one new workspace crate `crates/pmcp-team-servers` (0.x experimental, publishes after `pmcp-agent`) hosting four reference MCP servers with dev-grade backends, a pure composition-derivation function, an in-process team-wiring API, and an exportable wire-level conformance harness. There are essentially **no new third-party dependencies** — the phase is a composition of already-shipped in-repo machinery: `pmcp` (Client/Server/`ToolOutput::Result`/`InMemoryTaskStore`/streamable-HTTP), `pmcp-agent` (`AgentServer`, `CompletionSourceFactory`, `SlotResolver`), and `pmcp-package` (`TeamPackage`, wire-frozen 0.1). The one genuinely new external touch is `reqwest` (feature-gated, webhook channel only) and an optional keyword-scoring choice for mem-mcp.

The architecture is dominated by **one established pattern repeated four times**: the `TaskStore` backend-trait model — contract in the SDK, dev backend in the SDK, operated backend platform-side. `TeamFsBackend` and `TeamMemoryBackend` are new instances of it; approval-mcp reuses `InMemoryTaskStore` directly; team-mcp composes N `AgentServer` instances (Phase 108) reached over in-memory `DuplexTransport` + a `pmcp::Client` per member. The dynamic tool families (`team_mcp__<member>`, `team_approval__ask_<member>`) are **computed once from `TeamPackage` at wiring/build time** (D-07 snapshot-at-entry) — not truly per-request — so they can be registered as ordinary tools on `Server::builder()`.

The two artifacts the platform imports (the pure `derive_attachment` fn and the exportable conformance harness) are the phase's highest-leverage deliverables and must be property-tested and dependency-light. The single biggest execution risk is **surfacing top-level `_meta[related_task]` through team-mcp's member hop** (TEAM-05's whole point): it requires the member `AgentServer` to be driven as a **task-augmented** call, because the Phase 108 adapter returns the bare answer (not the task envelope with `related_task`) for non-task requests. See Pitfall 1.

**Primary recommendation:** Scaffold `crates/pmcp-team-servers` mirroring the `pmcp-sql-server`/`pmcp-tasks` shape (path deps on `pmcp`/`pmcp-agent`/`pmcp-package`, per-server + `conformance` + `webhook` features, feature-gated `[[bin]]` per server). Build every server on `Server::builder().tool(...)`, override `handle_output` → `ToolOutput::Result` for the `related_task`-bearing tools, and drive members/conformance through a real `pmcp::Client` over the existing `DuplexTransport` convention. Land `binding.yaml` + expanded fixtures beside the contract; confirm `pmat comply` CLI invocation early (it is NOT currently wired into any build target — see Open Question 1).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Composition-derived attachment (`derive_attachment`) | Pure library fn (SDK) | — | D-07: pure, exported, property-tested; policy that both SDK wiring and platform adopt |
| team-fs / mem-mcp dev backends | Backend-trait impl (SDK) | Filesystem / in-memory store | D-08/D-09/TEAM-04: dev fidelity via real `file://` + sibling `review/` dir + in-memory keyword index |
| approval-mcp task lifecycle | `InMemoryTaskStore` (pmcp) | Console/webhook channels | D-03..D-12: reuse `with_task_store()`; channels are pure notification transports |
| team-mcp member dispatch | `pmcp::Client` per member (MCP hop) | `AgentServer` (pmcp-agent) over `DuplexTransport` | D-13: real MCP hop is the TEAM-05 template; guards in `_meta` |
| Member agent LLM | `CompletionSourceFactory`/`SlotResolver` (pmcp-agent) | Ollama/Anthropic/FixedSource | D-15: reuse Phase 108 machinery; FixedSource for CI |
| HTTP exposure of any server | pmcp streamable-HTTP server (`http`/`streamable-http` feature) | stdio fallback | D-02: platform-endpoint shape; `x-pmcp-team-depth` header → `_meta` at edge (D-14) |
| Wire-level conformance | `pmcp::Client` over in-memory transport (SDK, exportable) | Fixtures in `contracts/` | D-17/D-19: "advertised == enforced" at the wire; platform points same runner at HTTP |

## Standard Stack

All dependencies below are **already in the workspace tree** — no new registry packages except the feature-gated `reqwest` (already used by `pmcp-agent`) and the optional `bm25` choice. Package Legitimacy Audit is therefore near-trivial (internal path deps).

### Core (crate dependencies)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pmcp` (path `../..`, `default-features = false`) | 2.17.0 | `Client`, `Server`/`ServerBuilder`, `ToolOutput::Result`, `CallToolResult`, `InMemoryTaskStore`/`TaskStore`, `TypedTool`, `RequestHandlerExtra` | Core SDK; `default-features=false` keeps build wasm-clean/reqwest-free per the `pmcp-agent` precedent `[VERIFIED: crates/pmcp-agent/Cargo.toml]` |
| `pmcp` (path, `features = ["streamable-http"]`) — binary target only | 2.17.0 | HTTP-first dev binaries (D-02) | `pmcp-sql-server` binary pins `features=["streamable-http"]` for exactly this `[VERIFIED: crates/pmcp-sql-server/Cargo.toml]` |
| `pmcp-agent` (path `../pmcp-agent`) | 0.1.0 | `AgentServer`/`AgentServerBuilder`, `CompletionSourceFactory`/`FixedSourceFactory`/`SamplingSourceFactory`, `SlotResolver` | The unit team-mcp composes N of; member LLM machinery (D-13/D-15) `[VERIFIED: crates/pmcp-agent/src/adapter/*]` |
| `pmcp-package` (path `../pmcp-package`, `= "0.1"` caret) | 0.1.x | `TeamPackage { members, human_roles, built_in_servers, limits, entry_point, finalizer_agents }` | Primary config format (D-03); wire-frozen — do NOT change serialization `[VERIFIED: crates/pmcp-package/src/package/team.rs]` |
| `serde` + `serde_json` (`preserve_order`) | 1.0 | Tool args/results, fixture parsing | Workspace convention `[VERIFIED: crates/pmcp-agent/Cargo.toml]` |
| `async-trait` | 0.1 | Object-safe async backend traits (`TeamFsBackend`, `TeamMemoryBackend`) | Workspace convention |
| `thiserror` | 2.0 | Backend/dispatch error enums | Matches connector-crate style `[VERIFIED: pmcp-sql-server]` |
| `tokio` | 1 (`macros`, `rt-multi-thread`) | Binary runtime; in-process wiring runtime | Standard |
| `tracing` | 0.1 | Console approval channel output; diagnostics | Workspace convention |
| `uuid` (`v4`) | 1.17 | Approval ids, task ids | Matches `pmcp-agent` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `clap` (`derive`, `env`) | 4 | Binary CLI (`--package`, `--port`, `--data-dir`, `--stdio`) | Binaries only; matches `pmcp-sql-server` `[VERIFIED]` |
| `tracing-subscriber` (`env-filter`) | 0.3.20 | Binary log init | Binaries only |
| `reqwest` (`default-features=false`, `rustls`, `json`) | 0.13 | Webhook (CI) approval channel POST (D-11) | Feature-gate behind `webhook`; `pmcp-agent` already uses this exact pin |
| `clap`-free lib | — | The wiring API + servers stay CLI-free | Keep Phase 110 CLI thin |

### Dev-dependencies
| Library | Version | Purpose |
|---------|---------|---------|
| `pmcp` (path, `features=["full"]`) | 2.17.0 | Client-side test harness (matches `pmcp-agent` dev-dep) |
| `tokio` (`full`) | 1 | Async tests |
| `proptest` | 1.7 | Property tests for `derive_attachment` (D-07) + BM25 invariants + guard invariants (ALWAYS requirement) |
| `pretty_assertions` | 1.4 | Readable fixture diffs |
| `tempfile` | 3 | team-fs dev-backend tests (workspace/review dirs) |
| `semver` | 1 | Constructing `TeamPackage`/`AgentPackage` versions in tests |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled keyword/BM25 scorer | `bm25 = "2.3.2"` crate `[ASSUMED — cargo search only]` | The crate bundles an optional embedder feature (the exact thing D-04/TEAM-04 forbid) and adds a dependency to a dep-cautious workspace; a ~80-line documented BM25/TF-IDF scorer is deterministic, zero-dep, and fully proptest-able. **Recommend hand-roll**; crate is a fallback only if scoring fidelity matters. |
| `InMemoryTaskStore` for approval-mcp | New bespoke approval store | `with_task_store()` + `InMemoryTaskStore` already give create→working→completed lifecycle observable via `tasks/get`/`tasks/result` `[VERIFIED: src/server/builder.rs]`. Reuse. |
| Truly per-request dynamic `tools/list` (`DynamicServerManager`) | Register roster tools once at build (`Server::builder().tool()`) | D-07 snapshot-at-entry: composition resolves once at wiring/entry, static dev config → compute the `team_mcp__<member>`/`team_approval__ask_<member>` set once. `DynamicServerManager` exists (`src/server/dynamic.rs`) but is unnecessary complexity here. |

**Installation:** No `cargo add` of registry packages required beyond feature-gated `reqwest` (already vendored via `pmcp-agent`). New crate is created by adding `crates/pmcp-team-servers` to the root `[workspace] members` list.

**Version verification (performed 2026-07-18):**
- `pmcp` root version `2.17.0` `[VERIFIED: root Cargo.toml]`
- `pmcp-agent` `0.1.0`, `pmcp-package` `0.1.x` `[VERIFIED: crate Cargo.toml files]`
- `bm25 = "2.3.2"`, `tantivy = "0.26.1"` exist on crates.io `[VERIFIED: cargo search]` — but NOT recommended (see Alternatives).

## Package Legitimacy Audit

> This phase installs **no new third-party registry packages** into the default build. Every core dep is an in-repo path crate or already vendored via `pmcp`/`pmcp-agent`. slopcheck was not run (no new external names to verify); the only external name that would be new — `bm25` — is **not recommended** and if adopted must be gated behind a `checkpoint:human-verify` task.

| Package | Registry | Age/Status | Source Repo | Disposition |
|---------|----------|-----------|-------------|-------------|
| `pmcp`, `pmcp-agent`, `pmcp-package` | in-repo path | this workspace | this repo | Approved (internal) |
| `reqwest 0.13` | crates.io | already vendored via `pmcp-agent` (feature-gated) | seanmonstar/reqwest | Approved (existing workspace dep, unchanged pin) |
| `clap`, `tokio`, `serde`, `serde_json`, `async-trait`, `thiserror`, `tracing`, `uuid`, `tempfile`, `proptest`, `semver`, `pretty_assertions` | crates.io | all already in workspace tree at pinned versions | — | Approved (existing workspace deps) |
| `bm25 2.3.2` | crates.io | NOT recommended (see Alternatives) | — | **REMOVED from recommendation** — hand-roll instead; if planner reconsiders, gate behind `checkpoint:human-verify` + run slopcheck |

**Packages removed due to recommendation:** `bm25` (dependency-hygiene + embedder-feature concern, not a slop verdict).
**Packages flagged as suspicious:** none.

## Architecture Patterns

### System Architecture Diagram

```
                         TeamPackage file (--package)
                                   │
                                   ▼
                    ┌──────────────────────────────┐
                    │  derive_attachment(&pkg)      │  ← D-07 pure fn, snapshot-at-entry
                    │  members.len()>=2 → team-mcp  │
                    │  human_roles≠∅   → approval   │
                    │  built_in_servers → opt-ins   │
                    └──────────────┬───────────────┘
                                   │ AttachmentSet
                                   ▼
        ┌──────────────── in-process wiring API (one tokio runtime) ───────────────┐
        │                                                                            │
        │   member A ──┐        member B ──┐          (each = Phase 108 AgentServer) │
        │  AgentServer │       AgentServer │                                         │
        │      ▲       │            ▲      │                                         │
        │      │ DuplexTransport pair (in-memory MCP, no sockets, D-04/D-13)         │
        │      │       │            │      │                                         │
        │   ┌──┴───────┴────────────┴──────┴──┐   ┌──────────┐  ┌──────────┐         │
        │   │  team-mcp: pmcp::Client/member  │   │ team-fs  │  │ mem-mcp  │         │
        │   │  team_mcp__<member> dispatch    │   │ fs__*    │  │ mem__*   │         │
        │   │  guards via _meta(x-depth,      │   │TeamFsBk. │  │TeamMemBk.│         │
        │   │  caller_id, ancestor_chain)     │   │ local dir│  │ BM25 mem │         │
        │   │  → ToolOutput::Result +         │   │ +review/ │  └──────────┘         │
        │   │    _meta[related_task]          │   └──────────┘                       │
        │   └─────────────────────────────────┘   ┌──────────────────────────┐      │
        │                                          │ approval-mcp             │      │
        │                                          │ resolve/get_approval +   │      │
        │                                          │ team_approval__ask_<m>   │      │
        │                                          │ InMemoryTaskStore        │      │
        │                                          │ console+webhook channels │      │
        │                                          └──────────────────────────┘      │
        └────────────────────────────────────────────────────────────────────────────┘
                                   │
     HTTP-first dev binaries (D-02): each server bound on pmcp streamable-HTTP port;
     x-pmcp-team-depth HTTP header ──► namespaced _meta on tools/call (D-14 edge map)

     Conformance runner (D-17/D-19, exportable): pmcp::Client ─initialize─►
       tools/list (exact set + schema equality) ─► tools/call per fixture ─► compare
```

### Recommended Project Structure
```
crates/pmcp-team-servers/
├── Cargo.toml              # per-server + conformance + webhook features; feature-gated [[bin]]s
├── src/
│   ├── lib.rs             # re-exports: derive_attachment, wiring API, backend traits, conformance
│   ├── compose/
│   │   ├── derive.rs      # derive_attachment(&TeamPackage) -> AttachmentSet (D-07, pure, proptest)
│   │   └── wiring.rs      # in-process TeamRuntime: wires servers+members over DuplexTransport (D-01/D-04)
│   ├── fs/
│   │   ├── backend.rs     # TeamFsBackend trait
│   │   ├── local.rs       # LocalDirBackend (file://, sibling review/ dir; D-08/D-09)
│   │   └── server.rs      # team-fs Server builder (11 fs__* tools)
│   ├── mem/
│   │   ├── backend.rs     # TeamMemoryBackend trait
│   │   ├── bm25.rs        # hand-rolled keyword/BM25 scorer (no embedder; TEAM-04)
│   │   └── server.rs      # mem-mcp Server builder (6 mem__* tools)
│   ├── approval/
│   │   ├── channels.rs    # ConsoleChannel + WebhookChannel (notify-only; D-10/D-11)
│   │   └── server.rs      # approval-mcp on InMemoryTaskStore; resolve/get + dynamic ask_<member> (D-12 subject ref)
│   ├── team/
│   │   ├── member.rs      # per-member pmcp::Client over DuplexTransport to an AgentServer (D-13)
│   │   ├── guards.rs      # depth/self-call/ancestor-cycle from _meta (D-14)
│   │   └── server.rs      # team-mcp; team_mcp__<member> → ToolOutput::Result + related_task (TEAM-05)
│   ├── conformance/
│   │   └── runner.rs      # exportable fixture runner over pmcp::Client (D-17/D-19)
│   └── bin/
│       ├── team_fs.rs     # required-features=["team-fs"]
│       ├── mem_mcp.rs     # required-features=["mem-mcp"]
│       ├── approval_mcp.rs
│       └── team_mcp.rs
├── examples/
│   └── doc_review_team.rs # D-16 E2E narrative on FixedSource (ALWAYS: runnable example)
└── tests/
    ├── conformance.rs     # drives runner against all four reference servers vs fixtures (TEAM-06)
    ├── derive_props.rs    # proptest for derive_attachment (D-07)
    └── small_team.rs      # "small team, one process" integration test (D-01)

contracts/
├── team-servers-v1.yaml           # rev additively for subject_task_id + _meta depth (D-12/D-14)
├── team-servers/binding.yaml      # NEW: equation → reference-fn bindings (D-18)
└── team-servers/fixtures/         # expand to every-tool + every-guard (D-20)
```

### Pattern 1: New feature-flagged crate with feature-gated dev binaries (TEAM-01)
**What:** A library crate exposing servers + wiring API, plus one `[[bin]]` per server gated by `required-features`.
**When to use:** All four dev binaries.
**Example:**
```toml
# Source: pattern verified against crates/pmcp-sql-server/Cargo.toml + crates/pmcp-tasks/Cargo.toml
[features]
default = ["team-fs", "mem-mcp", "approval-mcp", "team-mcp"]
team-fs = []
mem-mcp = []
approval-mcp = []
team-mcp = []
webhook = ["dep:reqwest"]           # CI approval channel (D-11)
conformance = []                    # exportable harness (D-17); on by default for this repo's tests
http = ["pmcp/streamable-http"]     # HTTP-first binaries (D-02)

[[bin]]
name = "team-fs"
path = "src/bin/team_fs.rs"
required-features = ["team-fs", "http"]
# ...one [[bin]] per server, each required-features-gated
```
Add `"crates/pmcp-team-servers"` to root `[workspace] members` (regular member — only root `pmcp` is clippy-gated, but ALWAYS requirements + `make quality-gate` still apply) `[VERIFIED: root Cargo.toml members list]`.

### Pattern 2: Backend-trait seam (TeamFsBackend / TeamMemoryBackend) — the TaskStore model (TEAM-02/TEAM-04)
**What:** An object-safe async trait defines the storage/IO contract; a dev impl lives in the SDK; the operated impl stays platform-side.
**When to use:** team-fs and mem-mcp.
**Example:**
```rust
// Source: pattern mirrors pmcp TaskStore (src/server/task_store.rs) + pmcp-agent seams
#[async_trait::async_trait]
pub trait TeamFsBackend: Send + Sync {
    async fn list(&self, path: &str) -> Result<Vec<Entry>, FsError>;
    async fn read(&self, path: &str) -> Result<Vec<u8>, FsError>;
    async fn write(&self, path: &str, bytes: &[u8]) -> Result<(), FsError>;
    async fn get_download_url(&self, path: &str) -> Result<String, FsError>; // file:// for LocalDirBackend (D-08)
    async fn sync_to_review(&self, path: &str) -> Result<(), FsError>;       // copy workspace→review/ (D-09)
    async fn sync_from_review(&self, path: &str) -> Result<(), FsError>;     // copy review/→workspace
    // ...11 fs__* operations total
}
```
The server layer maps each `fs__*` tool name → a backend call; unknown `fs__*` names error (never panic — contract invariant).

### Pattern 3: Approval on InMemoryTaskStore with notify-only channels (TEAM-03)
**What:** `team_approval__ask_<member>` mints a pending approval task; a channel (console/webhook) *notifies*; `resolve_approval` transitions it. One resolution path for both channels (D-10).
**When to use:** approval-mcp only.
**Example:**
```rust
// Source: with_task_store()/InMemoryTaskStore verified in src/server/builder.rs:766,839 + AgentServer usage
let task_store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
let server = Server::builder()
    .name("approval-mcp").version("0.1.0")
    .tool("resolve_approval", resolve_tool)   // UNNAMESPACED legacy (per contract)
    .tool("get_approval", get_tool)
    // one team_approval__ask_<member> per human_role, computed once at build (D-07)
    .task_store(task_store)
    .build()?;
```
Ask/resolve records carry optional `subject_task_id`/`subject_ref` (D-12), echoed by `get_approval`/`resolve_approval`. Webhook channel is a `reqwest` POST behind the `webhook` feature; console channel is a `tracing`/`println` notification. **No stdin prompting.**

### Pattern 4: team-mcp member dispatch — real MCP hop → ToolOutput::Result (TEAM-05)
**What:** Each roster member is a Phase 108 `AgentServer` run over one half of a `DuplexTransport` pair; team-mcp holds a `pmcp::Client` per member. `team_mcp__<member>` dispatch validates guards, forwards the call over the hop via the 109-00 `call_tool_with_task_and_meta` API (task + guard `_meta` in one call), then applies the explicit `MemberTaskForwarding` contract (re-emit a `ToolCallResponse::Result`, or `wait_for_task`+synthesize a `ToolCallResponse::Task`) and returns the member's `CallToolResult` via `ToolOutput::Result`, surfacing top-level related-task metadata under `RELATED_TASK_META_KEY`.
**When to use:** team-mcp only.
**Example:**
```rust
// Source: DuplexTransport (tests/common/duplex.rs) + AgentServer::run (crates/pmcp-agent/src/adapter/server.rs)
//         + ToolOutput::Result (src/server/mod.rs:246). Register team_mcp__<member> tools that override handle_output.
async fn handle_output(&self, args: Value, extra: RequestHandlerExtra) -> Result<ToolOutput> {
    let depth = parse_depth_strict(&extra)?;            // strict integer parse; garbage → Error (D-14)
    guard_depth(depth, self.max_team_depth)?;           // depth > max → Error
    guard_self_call(self.target_id, caller_id(&extra))?;// compare IDs not names
    guard_ancestor_cycle(self.target_id, ancestors(&extra))?;
    // full MCP hop as a TASK + guard-_meta call (109-00 API; see Pitfall 1)
    let resp = self.member_client.call_tool_with_task_and_meta(name, args, forward_meta).await?;
    let result: CallToolResult = match resp {                      // explicit MemberTaskForwarding contract
        ToolCallResponse::Result(r) => r,                          // re-emit (strip member _meta to related-task)
        ToolCallResponse::Task(t) => synthesize_from_task(&self.member_client, t).await?, // wait_for_task + synthesize
    };
    Ok(ToolOutput::Result(result))  // surfaces _meta[RELATED_TASK_META_KEY] (never a bare related_task)
}
```
`ToolOutput::Result` bypasses response middleware — the handler owns its own redaction (acceptable for a dev reference server; documented in `src/server/mod.rs`).

### Pattern 5: Wire-level conformance runner (TEAM-06)
**What:** A fixture-driven runner that drives ANY server impl through a real `pmcp::Client` over the in-memory transport: `initialize` → `tools/list` (assert exact advertised set + per-tool input-schema equality) → `tools/call` per fixture (assert `subset`-match against `expect.response`, incl. `_meta[related_task]` and error codes).
**When to use:** this repo's `tests/conformance.rs` AND exported (feature `conformance`) for the platform to import as a dev-dependency.
**Example:**
```rust
// Source: existing structural test tests/team_contracts_conformance.rs + DuplexTransport call_via_server()
// Fixture schema (verified): { schema_version:"1", case_id, server, request{name,arguments,_meta}, expect{outcome,match,response} }
pub async fn run_fixtures<S: Into<Server>>(server: S, fixtures_dir: &Path) -> ConformanceReport { /* ... */ }
```
Fixtures stay canonical in `contracts/team-servers/fixtures/`; embed into the exportable harness via `include_dir` OR a path parameter (Claude's discretion). Extend the Phase-107 representative set to **every tool + every guard** (D-20): ≥1 success per advertised tool, one error fixture per contract error path, and exact `tools/list` surface fixtures for all four servers.

### Anti-Patterns to Avoid
- **Truly-dynamic per-request `tools/list` for the roster families.** D-07 says composition resolves once at entry; for static dev config, compute `team_mcp__<member>`/`team_approval__ask_<member>` once at build. `DynamicServerManager` is unnecessary here.
- **Returning the member answer as `Payload` from team-mcp.** That drops `_meta[related_task]` — the whole point of TEAM-05. Use `ToolOutput::Result` (verbatim envelope).
- **Prompting on stdin for approvals.** D-10 forbids it; console is notify-only, resolution is always the `resolve_approval` tool.
- **Non-strict depth parsing.** The contract requires strict integer parse; garbage `x-pmcp-team-depth` must error, not default to 0.
- **Adding an embedder / vector dep to mem-mcp.** TEAM-04 and the milestone Out-of-Scope forbid it; keyword/BM25 only.
- **Changing `TeamPackage` serialization.** It is wire-frozen 0.1 (`pmcp-package`); consume it, never re-shape it.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| MCP client/server plumbing, initialize handshake | Custom JSON-RPC pump | `pmcp::Client`/`Server` + `DuplexTransport::pair()` | Verified in-repo convention (`tests/common/duplex.rs`); real wire path proves advertised==enforced |
| Task create→working→completed lifecycle for approvals | Bespoke approval store | `InMemoryTaskStore` + `.task_store()`/`with_task_store()` | Already gives observable lifecycle via `tasks/get`/`tasks/result` `[VERIFIED: src/server/builder.rs]` |
| Member agent loop, LLM sourcing, resume | New agent runtime | `pmcp-agent` `AgentServer` + `CompletionSourceFactory`/`SlotResolver`/`FixedSourceFactory` | Phase 108 delivered exactly this; D-13/D-15 ride it |
| Full CallToolResult envelope with `_meta[related_task]` | Manual JSON assembly + middleware bypass | `ToolOutput::Result(CallToolResult)` | Purpose-built variant (SEP-1686 / pmcp 2.12.0); the sanctioned TEAM-05 path |
| Secret handling / config-drift detection for member LLM | Ad-hoc env reads | `SlotResolver` + `RedactedSecret` + `detect_deviation` | Phase 108 D-15 machinery; secrets never hit logs (ASVS V7) |
| HTTP streaming server for binaries | Custom hyper server | pmcp `streamable-http` feature | `pmcp-sql-server` precedent; one flag |
| Conformance fixture schema | New format | Existing fixture schema + `tests/team_contracts_conformance.rs` structural checks | Phase 107 already froze `{schema_version, case_id, server, request, expect}` |

**Key insight:** This phase is ~90% *composition* of shipped machinery. The only genuinely new code is (1) two backend traits + their dev impls, (2) the pure `derive_attachment` fn, (3) the guard logic, and (4) the exportable runner. Everything else is wiring — resist rebuilding what `pmcp`/`pmcp-agent`/`pmcp-package` already provide.

## Common Pitfalls

### Pitfall 1: team-mcp member hop drops `related_task` unless dispatched as a task-augmented call
**What goes wrong:** The Phase 108 `AgentServer` tool handler returns the agent's **bare answer** (not a task envelope with `related_task`) when `!extra.is_task_request()` — this was a deliberate 108 fix (commit `fe7e68bf` "agent adapter returns the answer to non-task clients"). If team-mcp calls the member as an ordinary `tools/call`, the member never mints/surfaces a store task and the fixture expectation `_meta.related_task.taskId` will be absent, failing TEAM-05 and the `team_mcp__member.success` fixture.
**Why it happens:** `ServerCore::on_tool_call` only takes the TaskCreated path when `req.task.is_some()` (documented in `crates/pmcp-agent/src/adapter/server.rs:323`).
**How to avoid:** team-mcp must dispatch to the member via the 109-00 `Client::call_tool_with_task_and_meta` API (task-augmented AND carrying the guard `_meta`), then apply the explicit `MemberTaskForwarding` contract: re-emit a `ToolCallResponse::Result` via `ToolOutput::Result`, or on a `ToolCallResponse::Task` `wait_for_task` to terminal and synthesize a `CallToolResult` — either way surfacing top-level related-task metadata under `RELATED_TASK_META_KEY`. The plain `call_tool_with_task` (`src/client/mod.rs:624`) is INSUFFICIENT: it hardcodes `_meta: None` (cannot carry the D-14 guard `_meta`) and may return `ToolCallResponse::Task`. Confirm the member `AgentServer` is built `with_task_support(TaskSupport::Required)` (it is, per `server.rs:242`).
**Warning signs:** `team_mcp__<member>` fixture passes on content but `_meta.related_task` is null; `tasks/get` on the member shows no task.

### Pitfall 2: `pmat comply check` is not currently wired into any build target
**What goes wrong:** D-18 says "wire `pmat comply check`", but no Makefile/CI target invokes `pmat comply` today — even the existing `contracts/binding.yaml` (for `mcp-protocol-sdk-v1.yaml`) is validated by nothing in `make quality-gate`/CI.
**Why it happens:** The house "contract-first" rule in CLAUDE.md references `pmat comply check` aspirationally; it has not been operationalized in this repo's gate.
**How to avoid:** Treat "author `binding.yaml`" and "wire `pmat comply`" as **two** tasks. For the binding.yaml, mirror the existing `contracts/binding.yaml` shape (`version`, `target_crate: pmcp-team-servers`, `bindings[]` each `{contract, equation, function, module_path, signature, status, notes}`). For wiring, confirm the `pmat comply` CLI invocation and where it belongs (a new `make comply` target chained into `quality-gate`, and/or CI). See Open Question 1 — do this early so the binding format matches what `pmat comply` expects.
**Warning signs:** binding.yaml authored but nothing runs it; `pmat comply check` errors on an unexpected schema.

### Pitfall 3: dynamic tool-family surface must be EXACT for `tools/list` conformance
**What goes wrong:** `tools/list` conformance (D-19/D-20) asserts the advertised set is *exactly* the expected tools. team-mcp advertises one `team_mcp__<member>` per roster member; approval-mcp advertises `resolve_approval` + `get_approval` + one `team_approval__ask_<member>` per human role. An off-by-one (e.g. counting the entry_point/initiator, or namespacing `resolve_approval`) breaks the surface fixture.
**Why it happens:** The contract has sharp rules: `resolve_approval`/`get_approval` are **UNNAMESPACED legacy** names; the initiator/channel is "implicit and never counted" (D-05).
**How to avoid:** Compute the family from `TeamPackage` deterministically; add explicit `tools/list` surface fixtures per server (D-20) and assert exact equality, not subset, on the tool-name set.
**Warning signs:** extra/missing `team_*__` tool in `tools/list`; `resolve_approval` accidentally emitted as `team_approval__resolve`.

### Pitfall 4: guard state semantics — IDs not names, strict parse
**What goes wrong:** Self-call and ancestor-cycle guards compare **display names** instead of **stable member IDs**, or depth parsing tolerates non-integers.
**Why it happens:** Fixtures encode member IDs (`member-7`) distinct from tool names (`team_mcp__reviewer`); the contract explicitly says "compare ids not names" and "strict integer parse; garbage rejected."
**How to avoid:** Thread `caller_member_id` + ancestor chain through `_meta`; look members up by stable configured ID; parse `x-pmcp-team-depth` with `str::parse::<i64>()` and error on failure. Property-test all four guard paths (ALWAYS requirement).
**Warning signs:** `team_mcp__self-call.error` / `team_mcp__malformed-depth.error` fixtures fail; a same-name-different-id pair falsely rejected.

### Pitfall 5: `ToolOutput::Result` bypasses response middleware
**What goes wrong:** Handlers returning `ToolOutput::Result` skip redaction/sanitization/audit hooks and widget enrichment.
**Why it happens:** Documented, deliberate design (`src/server/mod.rs:246` — "handler owns its own redaction").
**How to avoid:** For these dev reference servers this is acceptable (no secrets in fs/mem/approval dev payloads), but the handler must not echo unsanitized member-provided `_meta` beyond `related_task`. Keep the re-emit tight: forward the member `CallToolResult` as-is only when it is the trusted in-process member hop.
**Warning signs:** unexpected `_meta` keys leaking through team-mcp; N/A for the dev backends.

## Runtime State Inventory

> Greenfield-additive phase (a new crate + new contract bindings + new fixtures). No rename/refactor of existing runtime state. Explicit check of all five categories:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — mem-mcp/team-fs dev backends are in-memory / temp-dir per run; approval uses `InMemoryTaskStore` (ephemeral). Verified: no persistent datastore introduced. | none |
| Live service config | None — no external service registration; HTTP binaries bind local ports at runtime. | none |
| OS-registered state | None — no OS-level task/service registration. | none |
| Secrets/env vars | Member LLM keys resolved via existing `SlotResolver` env conventions (Phase 108); webhook shared-secret header is per-run config, no new stored secret. Verified: no new secret key names baked into code. | none |
| Build artifacts | New crate → new `target/` build output + a new `Cargo.lock` entry; add to root `[workspace] members`. Publish-order entry added to CLAUDE.md release list (after `pmcp-agent`). | update root Cargo.toml + CLAUDE.md publish list |

## Code Examples

### Composition-derived attachment (D-07, pure + proptest)
```rust
// Source: rule from CONTEXT D-05/D-07; TeamPackage shape verified in crates/pmcp-package/src/package/team.rs
pub struct AttachmentSet { pub team_mcp: bool, pub approval_mcp: bool, pub opt_ins: Vec<ComponentRef> }

#[must_use]
pub fn derive_attachment(pkg: &TeamPackage) -> AttachmentSet {
    AttachmentSet {
        team_mcp: pkg.members.len() >= 2,             // N >= 2 AI agents
        approval_mcp: !pkg.human_roles.is_empty(),    // M >= 1 human member
        opt_ins: pkg.built_in_servers.clone(),        // team-fs/mem-mcp only if explicitly listed (D-06)
    }
}
// proptest invariants: N=1,M=0 → both false, opt_ins honored; N>=2 → team_mcp; M>=1 → approval_mcp.
```

### Feature-gated dev binary skeleton (D-02/D-03)
```rust
// Source: clap + streamable-http pattern verified against crates/pmcp-sql-server
#[derive(clap::Parser)]
struct Args {
    #[arg(long)] package: PathBuf,     // TeamPackage file (D-03 primary config)
    #[arg(long, default_value_t = 0)] port: u16,   // override only
    #[arg(long)] data_dir: Option<PathBuf>,        // override only
    #[arg(long)] stdio: bool,          // fallback transport (D-02)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Platform team-mcp bypasses MCP with raw JSON-RPC to reach members | Real MCP hop via `pmcp::Client` → member `AgentServer`, `ToolOutput::Result` + `related_task` | pmcp 2.12.0 (SEP-1686) + Phase 108 | TEAM-05 is the worked migration template replacing the bypass |
| `built_in_servers` = source of truth for server attachment | Composition-derived attachment (`members.len()`, `human_roles`); `built_in_servers` = opt-in extras | traces redesign 2026-07-18 (D-05) | SDK becomes the open, tested spec of the derivation rule |
| Structural-only contract conformance (`tests/team_contracts_conformance.rs`) | Wire-level runner over real `pmcp::Client` (advertised==enforced) | this phase (D-19) | Platform can point the same runner at an HTTP endpoint |

**Deprecated/outdated:** none introduced. `DynamicServerManager` (`src/server/dynamic.rs`) exists but is the wrong tool here (see Anti-Patterns).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `bm25 = "2.3.2"` / `tantivy = "0.26.1"` exist as described (cargo search only, not authoritative docs) | Standard Stack / Alternatives | Low — they are explicitly NOT recommended; hand-roll is the primary path |
| A2 | `pmcp::Client` exposes a task-augmented `tools/call` API sufficient to surface member `related_task` (exact method name TBD) | Pitfall 1 | HIGH — TEAM-05 depends on it; **verify in Wave 0** against the pmcp Client surface before building team-mcp |
| A3 | `pmat comply check` accepts a binding.yaml of the same shape as `contracts/binding.yaml` and can be invoked standalone | Pitfall 2 / Open Q1 | MEDIUM — affects D-18; confirm CLI early |
| A4 | HTTP edge mapping `x-pmcp-team-depth` header → `_meta` has no existing helper and must be written at the binary layer | D-14 | Low — additive edge code |

## Open Questions (RESOLVED)

> All three open questions were resolved during planning and folded into concrete plan tasks. Retained here with resolution markers for traceability.

1. **How is `pmat comply check` invoked and where should it be wired?** — **RESOLVED (folded into 109-08 Task 1/2).**
   - What we know: `contracts/binding.yaml` exists for `mcp-protocol-sdk-v1.yaml` with a clear schema (`version`, `target_crate`, `bindings[]{contract,equation,function,module_path,signature,status,notes}`). CLAUDE.md references `pmat comply check` as the house rule.
   - What's unclear: No Makefile/CI target runs `pmat comply` today; the exact CLI (`pmat comply check <binding.yaml>`?) and whether it resolves `function`/`module_path` against a compiled crate or source.
   - **Resolution:** 109-08 Task 1 probes `pmat comply --help` / `pmat comply check contracts/binding.yaml` against the *existing* binding to learn the CLI + schema BEFORE authoring `team-servers/binding.yaml`, and falls back to mirroring the existing `contracts/binding.yaml` shape byte-for-byte if `pmat` is absent. 109-08 Task 2 adds the `make comply` target chained into `quality-gate`, guarded by `command -v pmat` (warns, does not hard-fail) so a pmat-absent machine still passes the gate (D-18). No bindings authored blind.

2. **Exact task-augmented + guard-`_meta` client call for the member hop (A2).** — **RESOLVED (re-scoped in replan; see prerequisite plan 109-00).**
   - What we know: member `AgentServer` is `with_task_support(TaskSupport::Required)` and returns the task envelope only for task requests.
   - What's unclear: the precise `pmcp::Client` method to issue a task-augmented `tools/call` that ALSO carries the namespaced guard `_meta` (D-14 depth/ancestry), and how to guarantee top-level related-task metadata surfaces.
   - **Resolution:** The plain `Client::call_tool_with_task` (`src/client/mod.rs:624`) is **NOT sufficient** — it hardcodes `_meta: None` (so it cannot carry the D-14 guard `_meta`) and may return EITHER `ToolCallResponse::Result` OR `ToolCallResponse::Task` (it does not by itself guarantee a `CallToolResult` with top-level related-task metadata). The replan therefore adds prerequisite plan **109-00** (pmcp-core enablement): (a) `RequestMeta` gains a `#[serde(flatten)]` `other` map so arbitrary namespaced keys (`x-pmcp-team-depth`, ancestor chain) survive round-trip; (b) the request `_meta` is propagated into `RequestHandlerExtra.request_meta` so guards can read it; (c) a NEW client API `Client::call_tool_with_task_and_meta(name, args, meta: RequestMeta)` forwards task augmentation AND the guard `_meta` in one call. team-mcp (109-05) dispatches the member via `call_tool_with_task_and_meta`, then applies the EXPLICIT `MemberTaskForwarding` contract (109-01 `identity.rs`): `ToolCallResponse::Result(r)` → re-emit `r` (stripping member `_meta` to related-task only); `ToolCallResponse::Task(t)` → `Client::wait_for_task` to terminal and SYNTHESIZE a `CallToolResult` carrying related-task `_meta`. The related-task key is the SDK constant `RELATED_TASK_META_KEY` = `io.modelcontextprotocol/related-task` (`src/types/tasks.rs:9`), never a bare `related_task`. Assumption A2 is discharged via 109-00 + the 109-05 forwarding contract — NOT via `call_tool_with_task` alone.

3. **Contract YAML rev mechanics for the additive `subject_task_id` field + `_meta` depth doc (D-12/D-14).** — **RESOLVED (folded into 109-01 Task 4).**
   - What we know: metadata `version: 1.0.0`; additive change.
   - **Resolution:** 109-01 Task 4 (contract rev) bumps `team-servers-v1.yaml` `metadata.version` to `1.1.0` (additive minor), adds `subject_task_id`/`subject_ref` to the `approval_tool_surface` invariants (D-12), and documents the `x-pmcp-team-depth` `_meta` field in `team_dispatch_surface` (D-14) — all purely additive so the 19-static-tool-name / 2-dynamic-prefix conformance test stays green.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/cargo toolchain | build/test | ✓ | stable (matches CI `dtolnay/rust-toolchain@stable`) | — |
| `pmat` CLI | D-18 `pmat comply` + CI complexity gate | Assumed present (CLAUDE.md pins 3.15.0 in CI) | 3.15.0 | If absent locally, comply wiring can be authored + validated in CI only |
| Ollama / OpenAI-compat endpoint | Optional real-LLM member runs (D-15) | Not required for CI | — | **FixedSource** (D-16) — CI path needs no live LLM |
| `reqwest`/network | webhook approval channel (D-11) | vendored via `pmcp-agent`; network only at runtime | 0.13 | Console channel needs no network; webhook is feature-gated + opt-in |

**Missing dependencies with no fallback:** none — the entire CI/test path runs offline on FixedSource + in-memory transports.
**Missing dependencies with fallback:** live LLM (→ FixedSource); network for webhook (→ console channel).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]`/`#[tokio::test]` + `proptest 1.7` (property) + `cargo test` |
| Config file | `crates/pmcp-team-servers/Cargo.toml` (`[dev-dependencies]`, `[features]`) |
| Quick run command | `cargo test -p pmcp-team-servers` |
| Full suite command | `make quality-gate` (fmt/lint/build/test-all/pmcp-package-gate/audit/validate-always/purity-check) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TEAM-01 | crate builds; each dev binary compiles under its features | build/smoke | `cargo build -p pmcp-team-servers --all-features` | ❌ Wave 0 |
| TEAM-02 | team-fs `fs__*` surface + local backend (file://, review/ sync) | unit + integration | `cargo test -p pmcp-team-servers fs_` | ❌ Wave 0 |
| TEAM-03 | approval lifecycle on InMemoryTaskStore; console+webhook notify; resolve path | unit + integration | `cargo test -p pmcp-team-servers approval_` | ❌ Wave 0 |
| TEAM-04 | mem-mcp `mem__*` + BM25 keyword scoring invariants | unit + property | `cargo test -p pmcp-team-servers mem_` | ❌ Wave 0 |
| TEAM-05 | team-mcp member hop → `ToolOutput::Result` + `_meta[related_task]`; all four guards | integration + property | `cargo test -p pmcp-team-servers team_` | ❌ Wave 0 |
| TEAM-06 | wire-level conformance: exact `tools/list` + `tools/call` per fixture, all 4 servers | integration | `cargo test -p pmcp-team-servers --test conformance` | ⚠️ extends `tests/team_contracts_conformance.rs` |
| D-01 | "small team, one process" | integration | `cargo test -p pmcp-team-servers --test small_team` | ❌ Wave 0 |
| D-07 | `derive_attachment` rule (N/M matrix + opt-ins) | property | `cargo test -p pmcp-team-servers --test derive_props` | ❌ Wave 0 |
| ALWAYS | runnable example (doc-review E2E) | example | `cargo run -p pmcp-team-servers --example doc_review_team` | ❌ Wave 0 |
| ALWAYS | fuzz target (guard/depth parsing or BM25) | fuzz | `cargo fuzz run <target>` (add under `crates/pmcp-team-servers/fuzz` or root `fuzz`) | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p pmcp-team-servers <module>` (< 30s)
- **Per wave merge:** `cargo test -p pmcp-team-servers --all-features`
- **Phase gate:** `make quality-gate` green + (D-18) `pmat comply check` green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/pmcp-team-servers/Cargo.toml` + `src/lib.rs` — crate scaffold, feature flags, add to root workspace members
- [ ] `tests/derive_props.rs` — proptest harness for `derive_attachment` (D-07)
- [ ] `tests/conformance.rs` — wire-level runner shell (extends structural `tests/team_contracts_conformance.rs`)
- [ ] `tests/small_team.rs` — in-process wiring integration test (D-01)
- [ ] Spike (Open Q2): confirm pmcp `Client` task-augmented call for member hop **before** team-mcp tasks
- [ ] Spike (Open Q1): run `pmat comply` against existing binding.yaml to learn its schema **before** authoring team-servers/binding.yaml
- [ ] `crates/pmcp-team-servers/fuzz` (or root `fuzz` target) — ALWAYS fuzz requirement

## Security Domain

> `security_enforcement` not set in config → treated as enabled. Dev reference servers with a filesystem backend and an outgoing webhook — real (if modest) attack surface.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Dev servers, no auth surface (platform owns auth) |
| V3 Session Management | no | Stateless dev servers / in-memory |
| V4 Access Control | yes | team-mcp guards (depth/self-call/ancestor-cycle) are the access-control surface — enforce by stable ID; `TeamFsBackend` local dir must stay within its workspace root |
| V5 Input Validation | yes | Strict `x-pmcp-team-depth` integer parse; `fs__*` path validation (prevent `..`/absolute-path escape from the workspace/review dirs); schema-valid-before-side-effect (contract invariant) |
| V6 Cryptography | no | No crypto; webhook shared-secret is a plain header (no HMAC by D-11) — never hand-roll crypto here |
| V7 Secrets/Logging | yes | Member LLM keys via `RedactedSecret`/`SlotResolver` (never logged); webhook shared secret must not be logged |

### Known Threat Patterns for {Rust MCP reference servers}
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal via `fs__read`/`fs__write` (`../`, absolute paths) | Tampering / Information Disclosure | Canonicalize + assert containment within the configured workspace/review roots in `LocalDirBackend` |
| Unbounded team recursion (missing/forged depth) | Denial of Service | Strict depth parse + `depth > max_team_depth` reject + ancestor-cycle guard (contract-mandated) |
| Self-call / cycle loop between members | Denial of Service | Self-call guard (ID compare) + ancestor-chain guard |
| Secret leakage of member LLM key or webhook secret | Information Disclosure | `RedactedSecret` wrapper (Phase 108); no secret in `tracing` fields |
| SSRF via configured webhook URL | Server-Side Request Forgery | Webhook is dev/CI-only, opt-in, notify-only; document that the URL is operator-configured trusted input (platform keeps hardened egress) |
| Unsanitized `_meta` re-emit through team-mcp (`ToolOutput::Result` bypass) | Tampering | Forward only trusted in-process member envelope; keep re-emit tight (Pitfall 5) |

## Sources

### Primary (HIGH confidence)
- `contracts/team-servers-v1.yaml` — the four tool-surface equations, guard semantics, namespacing rules (read in full)
- `contracts/team-servers/fixtures/**` — fixture schema + representative cases (read all four server dirs)
- `tests/team_contracts_conformance.rs` — structural conformance base (read in full)
- `crates/pmcp-agent/src/adapter/server.rs` + `factory.rs` — `AgentServer`, `CompletionSourceFactory`, non-task-vs-task return path (read in full)
- `crates/pmcp-package/src/package/team.rs` — `TeamPackage` wire shape (read in full)
- `src/server/mod.rs:246` — `ToolOutput`/`ToolHandler::handle_output` (read the enum + trait)
- `src/server/builder.rs:762,839` — `with_task_store`/`task_store`/`TaskSupport` (grep-verified)
- `tests/common/duplex.rs` — `DuplexTransport`/`call_via_server`/`call_via_core` (read in full)
- `crates/pmcp-sql-server/Cargo.toml`, `crates/pmcp-tasks/Cargo.toml` — feature-gated crate + `[[bin]]` + isolation precedent (read)
- `contracts/binding.yaml` — binding.yaml schema for D-18 (read head)
- Root `Cargo.toml` (`[workspace] members`), `Makefile` (`quality-gate`) — integration points (read)

### Secondary (MEDIUM confidence)
- `.planning/phases/108-*/108-CONTEXT.md` context via memory + recent commits (`fe7e68bf` non-task return fix)

### Tertiary (LOW confidence)
- `cargo search bm25 / tantivy` — crate existence only, not recommended
- `pmat comply` CLI mechanics — inferred from CLAUDE.md + existing binding.yaml; NOT yet run (Open Q1)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every dep verified against in-repo Cargo.toml files; no new external names in the default build
- Architecture: HIGH — patterns lifted directly from shipped Phase 104-108 code and the frozen contract
- Pitfalls: HIGH for 1/3/4/5 (verified in code + contract); MEDIUM for 2 (pmat comply not yet operational)
- Security: MEDIUM — standard controls; filesystem + webhook surface reasoned from ASVS, not from an existing threat model doc

**Research date:** 2026-07-18
**Valid until:** 2026-08-17 (stable internal-SDK domain; re-verify only if `pmcp`/`pmcp-agent` versions bump or the contract revs)
