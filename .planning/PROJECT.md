# PMCP SDK Extensions

## What This Is

Extensions for the PMCP SDK: a `pmcp-tasks` crate implementing MCP Tasks (experimental spec 2025-11-25) with pluggable storage backends, and a complete MCP Apps developer experience — from `cargo pmcp app new` scaffolding through live preview with dual bridge modes to ChatGPT manifest generation and demo landing pages — enabling rich UI widgets served from MCP servers across ChatGPT, Claude, and other MCP clients.

## Core Value

Tool handlers can manage long-running operations through a durable task lifecycle (create, poll, complete) with shared variable state that persists across tool calls — giving servers memory without an LLM.

## Requirements

### Validated

<!-- v2.6 AI-Package Portability — shipped 2026-08-27 as tag v2.19.1 (14 crate versions live). -->
- ✓ **PKG-01** A config-only server packs end to end — `config.toml` + OpenAPI spec as vendor-media-type layers — v2.6
- ✓ **PKG-02** Dual-mode binary: embedded bootstrap bytes or referenced `BinaryRef { digest, media_type }` — v2.6
- ✓ **PKG-03** Baked-vs-slot split decided and machine-checkable (spec baked; endpoint/credentials/auth mode are slots) — v2.6
- ✓ **PKG-04** Package round-trips A→B with tool-list parity asserted on behaviour, not manifest shape — v2.6
- ✓ **PKGR-01** Publish ledger complete and machine-checked — the coverage gate discovers workspace-EXCLUDED crates (24→25 members) and asserts `pmcp-package` precedes all four consumers — v2.6

- ✓ Core protocol types (Task, TaskStatus, CreateTaskResult, etc.) matching MCP spec 2025-11-25 — v1.0
- ✓ Task status state machine with validated transitions (5 states, 46 transition tests) — v1.0
- ✓ TaskStore trait with pluggable storage backends (11 async methods) — v1.0
- ✓ In-memory storage backend for dev/testing (DashMap, atomic transitions) — v1.0
- ✓ TaskContext for ergonomic handler integration (typed accessors, status transitions) — v1.0
- ✓ PMCP extension: task variables as shared client/server scratchpad via `_meta` — v1.0
- ✓ Server/client task capability types and negotiation via `experimental.tasks` — v1.0
- ✓ Tool-level task support declaration (forbidden/optional/required) — v1.0
- ✓ TaskRouter for routing tasks/get, tasks/result, tasks/list, tasks/cancel — v1.0
- ✓ Task interception for task-augmented tools/call requests — v1.0
- ✓ Owner binding security (OAuth sub, client ID, session ID fallback) — v1.0
- ✓ TaskSecurityConfig with configurable limits (max tasks, TTL, variable size) — v1.0
- ✓ Comprehensive test suite: unit (200+), property (13), integration (11), security (19) — v1.0
- ✓ Basic tasks example (60_tasks_basic.rs) — v1.0
- ✓ Task-aware workflow prompts that create tasks and bind step progress — v1.1
- ✓ Partial server-side execution with automatic pause on unresolvable steps — v1.1
- ✓ Structured prompt reply conveying completed steps, remaining steps, and task ID — v1.1
- ✓ Step state tracking in task variables (standard schema: goal, steps, completed, remaining) — v1.1
- ✓ Client continuation pattern via direct tool calls guided by prompt reply — v1.1
- ✓ Working example demonstrating task-prompt bridge with multi-step workflow — v1.1
- ✓ Lower-level KV storage backend trait for pluggable persistence — v1.2
- ✓ GenericTaskStore that delegates to any StorageBackend implementation — v1.2
- ✓ InMemoryBackend refactored from existing InMemoryTaskStore — v1.2
- ✓ DynamoDB backend behind `dynamodb` feature flag (cloud-only tests) — v1.2
- ✓ Redis backend behind `redis` feature flag (proving the trait) — v1.2
- ✓ Automated feature-flag verification across all backend combinations — v1.2
- ✓ mcp-preview widget iframe rendering with working MCP bridge proxy — v1.3
- ✓ WASM in-browser MCP client with proxy/WASM toggle and standalone polyfill — v1.3
- ✓ Shared bridge library (App, PostMessageTransport, AppBridge) eliminating inline JS — v1.3
- ✓ File-based widget authoring with WidgetDir hot-reload and bridge auto-injection — v1.3
- ✓ `cargo pmcp app new` CLI scaffolding with documented bridge API and CSP helpers — v1.3
- ✓ ChatGPT-compatible ai-plugin.json manifest generation — v1.3
- ✓ Standalone demo landing pages with mock bridge — v1.3
- ✓ Chess, map, and dataviz MCP App examples shipping — v1.3
- ✓ 20 chromiumoxide CDP E2E browser tests across 3 widget suites — v1.3
- ✓ Book Ch 14 (Performance & Load Testing) — 961-line comprehensive chapter with CLI, config, metrics, CI/CD — v1.4
- ✓ Book Ch 15 Load Testing cross-reference section — v1.4
- ✓ Book Ch 12.5 (MCP Apps) rewritten with WidgetDir, cargo pmcp app, adapter pattern — v1.4
- ✓ Course Ch 18-03 hands-on load testing tutorial (952 lines) — v1.4
- ✓ Course Ch 12 Load Testing cross-reference section — v1.4
- ✓ Course Ch 20 sub-chapters rewritten with WidgetDir/mcpBridge/adapter paradigm — v1.4
- ✓ Course quizzes and exercises for load testing and MCP Apps content — v1.4
- ✓ Examples cleanup: 17 orphans registered, 63 files role-prefixed (s/c/t/m), accurate PMCP README index, protocol badge 2025-11-25 — v2.1 (Phase 65)
- ✓ Macros documentation rewrite: deleted deprecated `#[tool]`/`#[tool_router]`/stub `#[prompt]`/`#[resource]` from pmcp-macros (898 LOC, 46% of crate); rewrote pmcp-macros/README.md from scratch (355 lines, 5 compiling `rust,no_run` doctests for all four `mcp_*` macros) wired via `#![doc = include_str!("../README.md")]`; published pmcp-macros 0.5.0 and pmcp 2.3.0 with full MACR-02 migration guide — v2.1 (Phase 66)
- ✓ Client host surface: server→client `sampling/createMessage` (incl. tools/tool_choice), `elicitation/create`, `roots/list` via builder-registered handler registry; preflight sampling approval; registry-authoritative capability derivation — v2.4 (Phase 106, pmcp 2.16.0)
- ✓ `pmcp-package` adopted + wire-frozen (pinned-digest golden fixtures for all four kinds); team-server tool contracts as provable-contracts YAML + conformance fixtures — v2.4 (Phase 107)
- ✓ `pmcp-agent` 0.x loop crate: pure loop between effect seams (`CompletionSource`/`ToolInvoker`/`ConversationStore`), `SamplingSource` + feature-gated `OpenAiCompatSource`/`AnthropicSource`, agent-as-server adapter, tasks-aware `poll_decision` — v2.4 (Phase 108)
- ✓ `pmcp-team-servers` reference crate: team-fs, mem-mcp (BM25), approval-mcp, team-mcp (members as agent-as-server tools), additive pmcp-core namespaced `_meta`, in-process `TeamRuntime`, exportable wire-level conformance harness — v2.4 (Phase 109)
- ✓ cargo-pmcp 0.18.0 agent/team verbs: `agent new`/`agent dev`, `team dev`, `package inspect|capture` with version-pin tripwires — v2.4 (Phase 110)
- ✓ All 7 v2.4 crates published to crates.io as pmcp v2.17.0 (2026-07-19, PR #302/tag v2.17.0)
- ✓ docs.rs pipeline and feature flags: `Cargo.toml` `[package.metadata.docs.rs]` replaced `all-features = true` with explicit 15-feature list + dual targets (`x86_64-unknown-linux-gnu` + `aarch64-unknown-linux-gnu` for first-class ARM64/Graviton coverage); created crate-focused `CRATE-README.md` at repo root (171 lines, 18-row Cargo Features table) wired into `src/lib.rs` via `#![doc = include_str!("../CRATE-README.md")]` (matches Phase 66 pmcp-macros pattern, pulls DOCD-02 from Future Requirements into scope); fixed all 29 rustdoc warnings across 16 source files (+8 residual links orchestrator-applied) via the "demote to backticks" pattern; adopted `feature(doc_cfg)` (post-RFC 3631 now provides auto-cfg badging by default — original D-01 `doc_auto_cfg` flip was invalidated by Rust 1.92.0 upstream removal, amended mid-phase); added new `make doc-check` target (stable toolchain, D-16 feature list, TAB-indentation guarded) and CI `Check rustdoc zero-warnings` step inside the existing `quality-gate` job (deliberately NOT chained into local `make quality-gate` per D-27 to protect developer iteration speed); no pmcp version bump (D-28 — stays at 2.3.0, docs.rs re-renders on next unrelated release); human-verify nightly badge checkpoint APPROVED — v2.1 (Phase 67)

- ✓ Version plumbing & negotiation (VERS-01..09): per-request `_meta` clientInfo, `server/discover`, required `Mcp-Method`/`Mcp-Name`/`MCP-Protocol-Version` headers, error-code rename `-32002`→`-32602` — v2.5 (Phase 112)
- ✓ Stateless streamable-HTTP + multi-round-trip elicitation (HTTP-01..08): `initialize`/`initialized` and `Mcp-Session-Id` removal path, `InputRequiredResult`/`requestState`; era resolved from the RAW request body so every method — not just the three `_meta`-bearing ones — can be a v2 request — v2.5 (Phase 113)
- ✓ Tasks extension migration (TASK-01..06): `tasks/list` removed, `tasks/update` added, server-directed creation; the v1.x DynamoDB/Redis task-store investment survived as an API reshape only — v2.5 (Phase 114)
- ✓ JSON Schema 2020-12 + caching hints (SCHM-*): `structuredContent` as any JSON value, `ttlMs`/`cacheScope`, position-aware `$schema` traversal derived from the pinned meta-schemas rather than hand-kept keyword lists — v2.5 (Phase 115)
- ✓ Auth-hardening SEPs (AUTH-*): RFC 9207 `iss` validation, DCR `application_type`, the six SEPs — v2.5 (Phase 116)
- ✓ v1 severability (SMPL-01/02, CLNT-03/04): default-on `v1-compat` plus a `full-v2` feature set severing v1 machinery at compile time via signature-identical paired modules, so call sites carry no `#[cfg]`; CI severance gate is a real merge blocker — v2.5 (Phase 117)
- ✓ Client & agents on v2 (CLNT-01/02/05): era-aware client, `mcp-tester --dual-run` diffing both eras against an `era-deltas.yaml` baseline — v2.5 (Phases 117, 118.2)
- ✓ Conformance against the official suite (CONF-*): nine gaps G-1..G-9 found and closed; `Content::Resource` emits the spec shape on both eras via a tolerant `#[serde(try_from)]` reader — v2.5 (Phases 118, 118.1, 118.2)
- ✓ Docs in three shapes (DOCS-*, carried from v2.4 Phase 111): pmcp-book v2 migration chapter, README/CHANGELOG protocol-era rewrite, course alignment — v2.5 (Phase 119)
- ✓ Published as pmcp v2.19.0 (2026-08-20, PR #337/tag v2.19.0)

### Active

<!-- Milestone v2.7 SEP-2640 Skills Conformance & Positioning. v2.7 has no REQUIREMENTS.md yet — /gsd-new-milestone writes one. -->

**Carried over from v2.6 — both parked on the pmcp.run backend, not on this repo:**

- [ ] **PKGX-01**: A package carries a pmcp.run-issued attestation and verifies against pmcp.run's identity *(in-repo carriage half SHIPPED in v2.6; verification against pmcp.run's identity is parked — the backend endpoint does not exist)*
- [ ] **PKGX-02**: `cargo pmcp package pack | unpack | export | import` *(`save`/`load` SHIPPED complete in v2.6; `pull`'s live leg is `#[ignore]`d because the backend endpoint does not exist. Contract-verified, never executed against the platform)*

**v2.7 requirements:** not yet defined. The milestone's later phases (workflow→skill projection
`as_skill()`, `[[skills]]` digest pins on AgentPackage, tri-surface decision-matrix docs) are added
via `/gsd-phase add` as taken up — implementation-order items 23-25 in the spike-findings skill.

## Current Milestone: v2.7 SEP-2640 Skills Conformance & Positioning

**Goal:** Bring the shipped `skills` module (feature `skills`) into conformance with the CURRENT
SEP-2640 draft, then land the positioning work spikes 009-011 validated: workflow→skill projection,
digest-pinned agent skill consumption, and the tri-surface decision-matrix docs.

**Status (2026-09-02):** Phase 125 COMPLETE — the conformance fix shipped. `skills/list` and
`skills/get` are routed via the crate-private `InternalClientRequest` classifier (no new public
`ClientRequest` variant, honouring the 2.x exhaustive-enum promise), answered from the shipped
`Skills` registry with verbatim frontmatter and complete `{uri, digest, size}` manifests; the
nonstandard `skill://index.json` is retired; name-identity is validated at build. No further v2.7
phase has been added yet.

**Known residual risk accepted in phase 125 (CR-01):** a server that declares
`io.modelcontextprotocol/skills` and runs on **stdio** tears down the session on `skills/list` —
*silently*, with no JSON-RPC error to the client and no operator log (`crate::log` is a no-op stub
at `src/lib.rs:373-386`). Accepted because the SDK targets remote streamable-HTTP servers, stdio is
discouraged, and skills are strictly opt-in (`skills` is in neither `default` nor `full`). The
underlying receive-arm defect is pre-existing and general: ANY unroutable method frame kills a stdio
session, `server/discover` included.

**Non-goals:** a new public `ClientRequest` variant; client-side `skills/*` wrappers and
`resources/directory/read` (both explicitly deferred, never silently dropped).

### Future

- [ ] CloudFormation template integrating with cargo-pmcp deployment plugin system
- [ ] Integration with cargo-pmcp deployment plugin system (DynamoDB table via CFN stack)
- [ ] Cross-server task sharing on pmcp.run — shared TaskStore backend + OAuth sub owner binding enables multi-server workflow continuity
- [ ] DataSource::TaskVariable for steps to read values from task variable store
- [ ] Workflow resume from task state (re-invoke prompt with task ID to continue from last step)
- [ ] StepExecution user API for runtime step mode customization
- [ ] Examples: code mode, DynamoDB backend
- [ ] Loadtest provider trait abstraction (when second provider appears)
- [ ] Remote execution trigger from CLI (`cargo pmcp loadtest run --remote`)
- [ ] Result download/polling from CLI

## Current State

**Milestone v2.5 (MCP Spec 2026-07-28 v2 Support) SHIPPED 2026-08-22** — published as pmcp v2.19.0
(PR #337, tag v2.19.0). 11 phases, 176 plans, all verification `passed`. pmcp is now a dual-version
SDK: one server binary serves both MCP 2025-11-25 and 2026-07-28 clients through per-request
negotiation, with v2 as the strategic primary path (stateless/Lambda-first, Tasks-as-extension,
JSON Schema 2020-12) and v1 as a cleanly severable compatibility layer (`full-v2` cuts it at compile
time, gated in CI). The whole milestone stayed additive — a 2.x minor, no breaking change. The
official conformance suite drove nine real SDK gaps (G-1..G-9) to closure in Phases 118.1/118.2.

Closed as an **override_closeout**: 461 audit items were recorded as open debt rather than
acknowledged — see MILESTONES.md for why (two gsd-tools defects made acknowledgment unsafe).

Phase 117 complete (2026-08-09) — Agents, Tester & v1 Severability (v2.5): the `full-v2` severance cut landed, proven by execution rather than inspection. A default-on `v1-compat` feature plus a `full-v2` feature set (= `full` minus exactly `v1-compat`) now sever MCP v1 machinery at compile time via a PAIRED MODULE — `v1_session.rs` (real bodies) and `v1_session_off.rs` (signature-identical null twins) — so call sites carry NO `#[cfg]` at all and `session_id: Option<String>` keeps threading the POST pipeline, just always `None`. Delivered across 14 plans in 6 waves: (117-01..04) the severability primitive, v1 wire-byte and tester-report goldens pinned BEFORE any cut, and the pmcp-agent live-socket harness; (117-05..08) the CI severance gate made a real merge blocker, the paired-module mechanism proven, `UrlConnectorClientFactory` made era-aware (CLNT-03), and the `era-deltas.yaml` expected-difference baseline (CLNT-04); (117-09..11) the era chokepoints moved, the s53 v2 agent example, and `mcp-tester --dual-run` auto-detecting a dual-era server and diffing the two runs against the baseline; (117-12) twelve v1-only functions moved into the pair; (117-14) the CLIENT half severed too — a `full-v2` client stores no session id, echoes none back, sends no DELETE, writes no `Last-Event-ID`, leaving exactly ONE call-site `#[cfg]` in the 2,941-line transport; (117-13) the GET/DELETE verbs SPLIT rather than moved so a severed build answers 405 (routed and refused), never 404, plus the four gated config fields and a sunset policy that matches the code. Post-execution code review found 2 Critical defects the plans' own self-checks missed, both fixed and independently re-verified: CR-01, a silent `MCP-Protocol-Version` downgrade to `2025-03-26` because two PURE MESSAGE CLASSIFIERS (`is_initialize_request`, `extract_negotiated_version`) holding zero v1 state were wrongly twinned — un-twinned, with a header/body-agreement regression test and a negative control; and CR-02, a severed TEST build that did not compile at all (8 integration files, 6 era-neutral files, 3 examples) — gated, and the severed test command wired into CI via `scripts/run-severance-proofs.sh`, which fails on any ZERO-count run. Two false-green traps were recorded: a dev-dependency taking pmcp's DEFAULT features re-unifies `v1-compat` during `cargo test` (making a severed test report "0 tests, exit 0" — green while proving nothing; `cargo build -p pmcp` never sees dev-deps, `cargo test` does), and an `assert!(!cfg!(feature = "v1-compat"))` guard that could never fail because `cfg!` expands to a bool literal. Verified 4/4 must-haves by execution; CLNT-03, CLNT-04, SMPL-01, SMPL-02 traced; `cargo test -p pmcp --no-default-features --features full-v2` went from a hard build failure to 3,339 passing; default-build lib suite holds at exactly 1,880; `make quality-gate` exit 0. Known limitation documented, not hidden: `Client::initialize` is dual-era and `src/composition/mcp_client.rs` calls it under `full-v2`, so SMPL-01's "initialize" clause is met on the SERVER side only.

Phase 109 complete (2026-07-19) — Team Reference Servers (v2.4): the four team servers now exist as open reference implementations with dev-grade backends in one feature-flagged crate `crates/pmcp-team-servers` (workspace member, publish item 15). Delivered across 9 plans: (109-00) a tight, additive pmcp-core `_meta` enablement — `RequestMeta` gained a `#[serde(flatten)]` namespaced map (custom keys survive round-trip, typed fields don't leak), `RequestHandlerExtra.request_meta` propagation (ServerCore + high-level Server), and two forwarding client APIs `Client::call_tool_with_meta` / `call_tool_with_task_and_meta`; (109-01) the zero-SATD crate scaffold with promoted `DuplexTransport`, `PackageResolver`/`MemberId`/`MemberTaskForwarding` seams, atomic `derive_attachment`, and a cargo-fuzz sub-package; (109-02) **team-fs** (`TeamFsBackend` + `LocalDirBackend`, 11 `fs__*` tools, a PURE-LEXICAL path normalizer that proves containment before any I/O — no canonicalize-then-IO TOCTOU — plus symlink rejection); (109-03) **mem-mcp** (6 `mem__*` tools, a hand-rolled zero-dependency BM25 scorer with div-by-zero/IDF-floor/finiteness fixes and a deterministic id/clock seam); (109-04) **approval-mcp** (`InMemoryTaskStore` pending→resolved + explicit `ApprovalRepository`, service-owned/any-client-resolvable D-10, atomic first-writer resolution, notify-only console+webhook channels with bounded timeout and never-logged secret); (109-05) **team-mcp** — the worked migration template replacing the raw-JSON-RPC bypass: guard depth/caller/ancestor travel as namespaced `_meta` via 109-00, each member is a Phase 108 `AgentServer` over in-memory `DuplexTransport` via a per-member `pmcp::Client`, and the member hop forwards task+guard `_meta` with explicit `MemberTaskForwarding::Synthesize`; (109-06) `TeamRuntimeBuilder`/`TeamRuntime` proving "small team, one process" with cfg-gated fail-closed attachments and transactional startup; (109-07) an exportable wire-level conformance harness (fixture schema v2, `ConformanceTarget` over in-memory Client AND HTTP, semantic related-task assertions, negative harness tests) proving each server's tool surface against the Phase 107 PKG-03 contract fixtures; (109-08) a doc-review E2E example, four-binary subprocess smoke tests, and Makefile/CI `pmat comply` wiring. Code review (0 critical, 2 warning, 4 info) — WR-01 (tool-name slug collision → silent handler overwrite) and WR-02 (`LocalDirPackageResolver` unsanitized-name path traversal) are tracked non-blocking follow-ups. One gap-closure fix (`http` feature must also enable `pmcp/http` so the HTTP `ConformanceTarget` compiles under a dev-dependency-free `--all-features`/publish build — `cargo test`'s `full` dev-dep had masked it). Re-verified 7/7 must-haves (141-test crate suite green); TEAM-01..06 traced. Next: Phase 110 (cargo-pmcp Agent & Team verbs).

Phase 107 complete (2026-07-18) — Contracts & Package Format (v2.4, parallel track with 106): the portability contracts now exist versioned and wire-frozen with this repo as canonical home. `pmcp-package` was adopted (import + publish-hygiene, not a rewrite) as a standalone **workspace-excluded** crate at `crates/pmcp-package/` — publish-ready metadata (repo=paiml/rust-mcp-sdk, docs.rs table, dual LICENSE-MIT/APACHE + NOTICE, README wire-freeze policy), 130 tests, `cargo publish --dry-run` green, reached by CI/Makefile/release via `--manifest-path` and a dedicated `pmcp-package-gate` chained into `quality-gate` (PKG-01). Wire freeze has real teeth: golden fixtures for all four kinds (server/workflow/agent/team) with pinned `sha256:` digest constants + `include_bytes!` canonical-byte snapshots (17 `digest_stability` tests; tamper flips digest → CI red) — STATE-1 (publish-readiness) verified; STATE-2 (actual crates.io publish) is a release-tag checkpoint tracked in `107-HUMAN-UAT.md` (PKG-02 pending on that). The four team-server tool surfaces (19 static `fs__*`/`mem__*`/approval names + 2 dynamic prefixes) are captured as versioned provable-contracts YAML `contracts/team-servers-v1.yaml` with 13 conformance fixtures + `tests/team_contracts_conformance.rs` (PKG-03). Code review (0 critical, 3 warning, 2 info): WR-01 fixed — `pmcp-package` release publish moved to the end (no in-repo consumers yet, so its experimental publish must not gate the core SDK release); WR-02 fixed (`.pv/` lint cache untracked + gitignored); WR-03/IN-01/IN-02 left as advisory test-design follow-ups. Verified 3/3 must-haves. Next: Phase 108 (`pmcp-agent` loop crate) — depends on 106 + 107.

Phase 106 complete (2026-07-17) — first phase of milestone v2.4 (Agents & Teams): the pmcp `Client` gained a host surface (`pmcp::client::host`) answering server→client `sampling/createMessage` (incl. tools/tool_choice), `elicitation/create`, and `roots/list` through a builder-registered handler registry; preflight sampling approval (deny-before-LLM, denial-of-wallet safe) + optional result review; registry-authoritative capability derivation (anti-capability-lie, caller sub-detail preserved); `Root`/`ListRootsResult` relocated to target-agnostic `src/types/roots.rs` (wasm-clean host module); inbound `ping` answered per spec; legacy `Client::create_message` documented as the "LLM-server pattern" AND fixed (was dead — missing `assert_capability` sampling arm, caught by phase code review); routing fuzz target; pmcp 2.15.0→2.16.0 + cargo-pmcp 0.17.4 scaffold pin. Verified 16/16 must-haves, `make quality-gate` green. Deferred: D-106-A (`Server::run` serialized loop deadlocks on in-tool sampling — Phase 108 needs this), stale book line re create_message fix (Phase 111). Phases 107 (contracts) next — parallelizable track already planned at roadmap level.

Phase 104 complete (2026-07-05): Task-Augmented Tool Results DX (SEP-1686 junction) — `ToolOutput::Result` verbatim pass-through closes the `Server`/`ServerCore` double-wrap junction (TOUT-01, D-04a response-middleware bypass locked), the high-precision double-wrap tripwire WARNs/debug-asserts at both Payload wrap sites with per-tool `suppress_double_wrap_check()` opt-out (TOUT-02), the client gains `TaskMetadata` + `CallToolResult::with_related_task()`/`related_task()` + wasm-safe `Client::wait_for_task()` (TOUT-03), and the migration path ships as runnable example s47 + live-HTTP `_meta` acceptance test + design doc + pmcp-book Ch 12.7 (TOUT-04). Code review (1 Critical + 4 Warnings) fully fixed incl. the `wait_for_task` `InputRequired` hang; re-verified 7/7; `make quality-gate` + `make doc-check` green (RUSTSEC-2026-0194/0195 quick-xml advisories via umya-spreadsheet documented in audit ignores pending upstream fix).

Milestone v2.3 (Excel-as-Configuration MCP Servers) — Phase 96 complete (2026-06-15), closing the v2.3 governed-Excel workbook track: (WBCL-05) `cargo pmcp new --kind workbook-server` scaffolds a runnable, purity-safe Shape B crate (`default-features = false`, `workbook-embedded` + `http`, embedded tax-calc@1.1.0 bundle, drift-locked `main.rs`, publish-safe `include_dir!` assets) mirroring `--kind sql-server`; (WBDL-02) a workbook self-declares its dialect version in a reserved `pmcp_dialect_version` named range, validated fail-closed (different major or newer-than-supported → typed `CompileError`, absent → baseline/no-error) via a shared `validate_dialect_version_step` wired into BOTH the seed and gated-update compile lanes (sibling `dialect_version.rs`, `version.rs` byte-unchanged), with fuzz + property + a runnable example; (WBEX-01) a second non-lighthouse synthetic loan/mortgage workbook (whitelist-legal VLOOKUP/INDEX-MATCH rate-tier DAG, no PMT/POWER, multi-output) compiles and serves end-to-end with its OWN `get_manifest`/`tools/list` schema (loan keys present, tax keys absent, key sets disjoint, the five generic tool names unchanged, zero per-workbook served Rust) — the manifest-driven §5 generalization proven via a new `in_*`/`out_*` named-range convention; (WBEX-02) an 8-quirk Excel corpus in both layers (scalar_eval units + penny-reconcile fixtures via `within_tol`, 1900-leap resolved as DAG-expressible with no new dialect functions). A reusable `#[cfg(test)]` rust_xlsxwriter fixture author with genuine Excel identity + production-refusal provenance assertions retired the .xlsx-authoring landmine. Cross-AI plan review (Gemini/Codex) + a code review (HI-01 gated-update-lane gap found and fixed) applied; verified 4/4 success criteria, all four req IDs (WBCL-05, WBDL-02, WBEX-01, WBEX-02) traced; `make quality-gate` + `make purity-check` green. Phase 95 complete (2026-06-14): `pmcp-workbook-server` Shape A pure-config binary stands up a live MCP server from a compiled bundle alone (no user Rust), a field-for-field re-skin of `pmcp-sql-server` (lib `run`/`serve`/`run_serving` + thin `main.rs` shim, `RunError` → non-zero exit, `LocalDirSource` selected from CLI args, `--bundle-id` fail-closed boot-integrity assertion). Full test trio (assemble surface / ephemeral-port HTTP smoke / mcp-tester parity through the real `run_serving` path) + proptest fuzz of the `--bundle-id` guard + reader-free purity-gate assertion for the served cone + CLAUDE.md slot-9a publish wiring; verified 3/3 success criteria, WBCL-06 satisfied. Phase 94 (CLI subcommands + `pmcp.toml`) complete prior. Phase 93 complete (2026-06-12): `pmcp-workbook-compiler` ports the full offline pipeline (ingest → lint → manifest synth → formula parse → DAG compile → penny-reconcile → artifact emit → promote gate) with `umya` isolated to this crate (purity gate extended + green). §5 generalization fixes landed at extraction (not copied): `synthesize()` is fully workbook-driven (`build_reference_manifest` gone), symmetric change-class classification (CR-01), versioned non-overwriting bundle writes (CR-02), enum-tiering correctness (WR-01), and umya fabricated-provenance refusal (WBCO-07). Change-class + auto-derived golden-corpus promote gate with the `--accept` BA approval flow shipped. Code-review BLOCKER CR-01 (publishable `trusted-fixture` feature could disable provenance refusal via Cargo feature unification) fixed: override is now `#[cfg(test)]`-only, golden proof relocated in-crate, `oracle/non-excel-app` removed from softenable set. Verified 16/16 must-haves; all 15 req IDs (WBCO-01..07, WBGV-01..07, WBDL-03) traced; `make quality-gate` + `make purity-check` green; 247 compiler-crate tests pass. Phase 92 complete (2026-06-11): bundle contract frozen from the consumer side — `BundleSource` trait (local-dir + embedded) with boot-time integrity gate in `pmcp-workbook-runtime`, plus the generic manifest-driven `workbook` feature module in `pmcp-server-toolkit` (all five tools incl. `workbook://` render resource, fail-closed validation, tax-calc@1.1.0 golden bundle, HTTP example, integration suite); verification gaps CR-01 (caller-input honoring) and the three fail-open paths (WR-02/WR-04/WR-07) closed by gap plans 92-06/92-07, re-verified 5/5 (WBSV-01..09). Phase 91 complete (2026-06-10): reader-free `pmcp-workbook-runtime` leaf crate, `pmcp-workbook-dialect` contract crate (WBDL-01), and the fail-closed purity gate (WBRT-04). WBDL-03 (linter) re-mapped to Phase 93 per D-02 (delivered). Next: Phase 94 (CLI subcommands + `pmcp.toml` — `cargo pmcp compile-workbook`/`lint-workbook`/`emit-bundle` thin shells over the compiler + the gated `--accept` flow). v2.2 (Configuration-Only MCP Servers / SQL + OpenAPI toolkits) substantially complete: phases 82–90.2 delivered the toolkit-core lift, SQL connectors (postgres/mysql/athena/sqlite), all four DX shapes, and the OpenAPI built-in server with advanced examples; remaining v2.2 items are experimental 999.x phases. All prior milestones (v1.0–v2.1) shipped.

v2.3 extracts the proven Excel-as-Configuration compiler from the `ai-on-cloud/towelrads-quote-pricing` lighthouse (its milestone v0.5.0 — phases 7–14 — is complete: golden quote reconciled to ±£0.01, ~730 workspace tests, snapshot tests on `tools/list` schemas, promote-gate integration tests including a real BA `--accept` flow, crate-level `#![deny(clippy::unwrap_used, expect_used, panic)]` on value paths). The extraction is a full end-to-end cut: runtime + compiler + CLI subcommands + generic served-tool toolkit module, with the known generalization gaps (RFC §5) redesigned rather than copied.

**Shipped milestones:**
- v1.0: MCP Tasks Foundation (types, store, server integration)
- v1.1: Task-Prompt Bridge (workflow execution, handoff, continuation)
- v1.2: Pluggable Storage Backends (DynamoDB, Redis, feature flags)
- v1.3: MCP Apps Developer Experience (preview, WASM, authoring, publishing, examples, E2E)
- v1.4: Book & Course Update (load testing docs, MCP Apps chapter refresh, quizzes, exercises)
- v1.5: Cloud Load Testing Upload (loadtest config upload, OAuth for load testing)
- v2.0: Protocol Modernization (protocol 2025-11-25, Tower middleware, conformance, proc macros, pentest, secrets)
- v2.1: Examples & Docs Hygiene (examples cleanup, macros rewrite, docs.rs pipeline)
- v2.2: Configuration-Only MCP Servers (SQL + OpenAPI toolkits, four DX shapes, pmcp-server-toolkit)
- v2.3: Excel-as-Configuration MCP Servers (workbook runtime/compiler/CLI, purity gate, Shape A/B) + Tasks DX arc (phases 101–105: tools-as-Tasks, HTTP tasks, SEP-1686 task-augmented results, poll-decision classifier)
- v2.4: Agents & Teams — SDK Extraction (phases 106–110: client host surface, pmcp-package contracts, pmcp-agent loop crate, pmcp-team-servers, cargo-pmcp agent/team verbs; published as pmcp 2.17.0). Phase 111 (docs in three shapes) folded into v2.5.
- v2.5: MCP Spec 2026-07-28 (v2) Support (phases 112–119: version plumbing, stateless HTTP + multi-round-trip elicitation, Tasks-as-extension, JSON Schema 2020-12 + caching hints, auth-hardening SEPs, v1 severability, official-suite conformance, docs in three shapes; published as pmcp 2.19.0)

### Out of Scope

- Task status notifications — skip for now, rely on polling only (validated by v1.0: polling works well)
- Bounded blocking on tasks/result — polling-only behavior
- Redis Cluster support — single-node sufficient (validated by v1.2: single-node Redis backend shipped)
- Task progress streaming via SSE — future phase
- Moving types into core pmcp crate — wait for spec stabilization
- Namespaced variable keys — flat keys with convention recommendation in docs (validated by v1.0: flat keys sufficient)
- Variable size enforcement per-backend — trait-level configurable limit works (validated by v1.0)
- Automatic client execution — MCP clients decide when/how to call tools; server cannot drive client
- Per-step task statuses — single task status with variable-level step tracking suffices (validated by v1.1)
- Workflow branching/conditionals — sequential-only; branching is a different workflow engine
- DynamoDB Local / docker-based testing — cloud-only DynamoDB in CI

## Context

Shipped v2.5 (2026-08-22) as pmcp v2.19.0. The v2.5 range alone changed 171 code files
(+59,749/−2,586) across `src/`, `crates/`, `cargo-pmcp/`, `examples/` and `tests/`; 427 files and
+159,043 lines including planning artifacts. Timeline on `main` 2026-08-07 → 2026-08-20 (the work
was developed on a feature branch and squash-merged as PR #337, so `main` shows 11 commits for a
milestone scoped from 2026-07-22).

Earlier baseline: shipped v1.4 with ~41,000+ Rust LOC across the workspace (v1.0: ~11,500 + v1.1: +10,697 + v1.2: +9,802 + v1.3: +9,197) plus 8,140 lines of documentation content in v1.4.
Tech stack: `pmcp-tasks` (serde, async-trait, dashmap, uuid, chrono, tokio, parking_lot; optional: aws-sdk-dynamodb, redis) + `pmcp` core (protocol types, ServerCore routing, workflow system, MCP Apps) + `cargo-pmcp` (CLI tooling) + `mcp-preview` (browser preview) + `mcp-e2e-tests` (chromiumoxide CDP) + `packages/widget-runtime` (TypeScript bridge library).

- The MCP Tasks spec is experimental (2025-11-25). Most MCP clients don't support it yet, so the feature is optional and isolated in `pmcp-tasks`.
- PMCP extends the minimal spec with task variables — a shared scratchpad visible to both client and server via `_meta`. This is the key innovation for servers without LLM capabilities.
- v1.1 bridges the `SequentialWorkflow` system with tasks: workflows pause mid-execution and the client continues via structured handoff guidance.
- v1.2 introduced pluggable storage backends: `StorageBackend` KV trait with `GenericTaskStore<B>` centralizing all domain logic. Three backends ship: `InMemoryBackend` (default), `DynamoDbBackend` (feature-flagged), `RedisBackend` (feature-flagged).
- v1.3 shipped the complete MCP Apps developer experience: `mcp-preview` with dual proxy/WASM bridge modes, `WidgetDir` file-based widget authoring with hot-reload, `cargo pmcp app new` scaffolding, `cargo pmcp app build` for manifest+landing generation, and 3 example apps (chess, map, dataviz) with 20 E2E browser tests.
- MCP Apps is an OpenAI extension (ChatGPT Apps / SEP-1865) adding rich HTML UI widgets to MCP servers. PMCP SDK supports multiple MIME types: `text/html+skybridge` (ChatGPT), `text/html+mcp` (standard MCP Apps), `text/html` (MCP-UI). Core types and adapters are in `src/types/mcp_apps.rs` behind `mcp-apps` feature flag.
- The shared bridge library (`packages/widget-runtime/`) provides App, PostMessageTransport, and AppBridge classes with TypeScript type definitions, compiled to ESM/CJS.
- Detailed design document: `docs/design/tasks-feature-design.md`

## Constraints

- **Isolation**: Must be a separate crate (`pmcp-tasks`) — experimental feature cannot destabilize core SDK
- **Spec compliance**: Protocol types must match MCP 2025-11-25 schema exactly
- **Feature gating**: DynamoDB backend behind `dynamodb` feature flag
- **Compatibility**: No breaking changes to existing `pmcp` crate API (validated: v1.0 and v1.1 only additive changes)
- **Testing**: Real DynamoDB in CI (cloud test table), no local docker dependency
- **Variable limits**: Trait-level configurable size limit enforced across all backends

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Separate crate (`pmcp-tasks`) | Experimental spec isolation, independent versioning | ✓ Good — clean separation, pmcp core unchanged for non-task users |
| Polling-only for tasks/result | Simpler implementation, Lambda-compatible, spec allows it | ✓ Good — 11 integration tests validate polling flow |
| Trait-level variable size limits | Consistent enforcement across backends, not just DynamoDB's 400KB | ✓ Good — `StoreConfig.max_variable_size_bytes` enforced in InMemoryTaskStore |
| Skip notifications for now | Simplifies initial implementation, polling sufficient | ✓ Good — TaskStatusNotification type defined but not wired; ready for v2 |
| Flat variable keys | Simplicity over structure, convention in docs | ✓ Good — top-level injection into `_meta` works cleanly |
| Capabilities via experimental field | Spec-compliant for experimental features, migrate when stabilized | ✓ Good — `experimental.tasks` auto-configured by `with_task_store()` |
| serde_json::Value for TaskRouter | Avoid circular crate dependency (pmcp-tasks depends on pmcp) | ✓ Good — clean trait boundary, pmcp has zero knowledge of pmcp-tasks types |
| DashMap for InMemoryTaskStore | Matches SessionManager pattern in existing codebase | ✓ Good — concurrent access tested with 10-thread proptest |
| Owner ID as structural key | NotFound on mismatch (never OwnerMismatch) — no info leakage | ✓ Good — 19 security tests verify isolation |
| TaskRouter in pmcp, impl in pmcp-tasks | One-directional dependency, builder accepts Arc\<dyn TaskRouter\> | ✓ Good — example only needs pmcp-tasks imports |
| Composition over modification (v1.1) | TaskWorkflowPromptHandler wraps WorkflowPromptHandler without changing it | ✓ Good — zero backward-compatibility issues, all existing tests pass unchanged |
| Hybrid handoff format (v1.1) | `_meta` JSON for machine parsing + natural language for LLM clients | ✓ Good — works with any MCP client regardless of structured output support |
| Fire-and-forget continuation (v1.1) | Continuation recording never fails the tool call | ✓ Good — tool results always returned to client; recording is best-effort |
| Cancel-as-completion (v1.1) | `tasks/cancel` with result transitions to Completed, not Cancelled | ✓ Good — enables clean workflow completion after all steps done client-side |
| Local mirror types (v1.1) | PauseReason/StepStatus mirrored in pmcp to avoid circular dependency | ✓ Good — same approach as TaskRouter; clean trait boundary preserved |
| Runtime best-effort execution (v1.1) | Dropped StepExecution enum; steps execute what they can at runtime | ✓ Good — simpler than static classification; PauseReason captures why stops |
| KV StorageBackend with GenericTaskStore (v1.2) | Domain logic once, backends are dumb KV stores | ✓ Good — 3 backends share identical domain logic; zero divergence |
| CAS in trait from day one (v1.2) | Retrofitting after backends exist would require rewriting every backend | ✓ Good — all 3 backends implement put_if_version atomically |
| Canonical JSON serialization (v1.2) | Prevents format divergence across backends | ✓ Good — identical round-trip behavior regardless of backend |
| Composite string keys (v1.2) | `{owner_id}:{task_id}` for universal backend support | ✓ Good — maps naturally to DynamoDB partition keys and Redis key prefixes |
| Feature-flagged backends (v1.2) | DynamoDB/Redis behind feature flags, InMemory always available | ✓ Good — zero-cost default, opt-in for production backends |
| Lua scripts for Redis CAS (v1.2) | Atomic check-and-set without WATCH/MULTI race conditions | ✓ Good — 19 integration tests verify atomicity |
| Session-once RwLock for MCP proxy (v1.3) | Resettable session support for reconnect button; OnceCell cannot reset | ✓ Good — session persists across requests, reconnect works |
| Bridge-first approach (v1.3) | Preview bridge is the load-bearing dependency for all downstream phases | ✓ Good — phase ordering validated as correct |
| Extract shared library after proving (v1.3) | Build 2 bridge implementations before extracting widget-runtime.js | ✓ Good — abstraction covers both proxy and WASM cases |
| App class uses document.referrer for origin (v1.3) | Prevents CVE-class wildcard postMessage vulnerability | ✓ Good — security fix for the blocked concern |
| WidgetDir disk reads on every call (v1.3) | Zero-config hot-reload without file watchers | ✓ Good — simplest approach, no caching bugs |
| chromiumoxide over Playwright (v1.3) | Pure Rust E2E tests, no Node.js dependency | ✓ Good — 20 tests pass, auto-downloads Chromium |
| Standalone examples (workspace exclude) (v1.3) | Avoids feature flag unification conflicts | ✓ Good — each example builds independently |
| rmcp parity research scoped to ergonomics-only with severity-graduated proposals (Phase 69) | Avoid overlap with Phase 68 polish; produce actionable follow-on phases not vague gap reports | ✓ Good — 4 High-severity gaps surfaced, 3 follow-on proposals (PARITY-HANDLER/CLIENT/MACRO-01) with concrete plan-count estimates |
| Dual-version stack over hard v2 cutover (v2.5) | One binary serves both eras via per-request negotiation; no forced client migration | ✓ Good — whole milestone stayed additive (2.x minor), zero breaking changes |
| Read the era from the RAW request body, not typed structs (v2.5, D-113-D) | Adding `_meta` to the five list-shaped request types forced a MAJOR semver bump; the raw read needs zero public API change | ✓ Good — reverted the typed attempt, one era-detection path instead of two that disagreed; `tools/list` can be a v2 request |
| Paired modules for v1 severance, not `#[cfg]` at call sites (v2.5) | `v1_session.rs` + signature-identical `v1_session_off.rs` null twins keep call sites clean | ✓ Good — 2,941-line transport left with exactly one call-site `#[cfg]`; CI gate blocks merge on regression |
| Derive keyword lists from the pinned meta-schemas (v2.5, Phase 115) | Hand-kept lists silently omitted `dependencies`, so a subschema went unrewritten with no warning | ✓ Good — six keywords now derived and held by a source-text drift gate (`keyword_list_mirrors.rs`) |
| Assert conformance on behaviour, not manifest structure (v2.5→v2.6) | Manifest schema is expected to churn; the E2E is the asset, not the API | — Pending — carried into v2.6 Phase 121 as the tool-list-parity round-trip |
| Require a source-reading plan reviewer (v2.5, Phases 116/118/119) | Checker-approved plans carried HIGH defects that only a source-reading reviewer found | ✓ Good — recorded as a standing practice; prose-only reviewers emit checkably-false findings |

---
## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-09-02 after milestone v2.7 opened and **v2.6 (AI-Package Portability) COMPLETED and archived** — 5 phases / 32 plans / 77 tasks, shipped 2026-08-27 as tag `v2.19.1` (14 crate versions live on crates.io). Closed as an `override_closeout`: 5 of 7 requirements Complete, with PKGX-01/02 still Pending because both are parked on a pmcp.run backend that does not exist yet — exactly as scoped at milestone open, not a slip. 486 audit items carried forward UNACKNOWLEDGED (453 belong to already-archived milestones, 11 to phase 125/v2.7, only 13 genuinely v2.6-scoped); acknowledgment refused 11 of the 13 because `audit-open`'s scanner emits table rows and sub-bullets while its `acknowledge` writer only matches `##` headings — the SAME defect recorded at the v2.5 close. The release itself did NOT ship cleanly: run #33122173433 failed on a crates.io ownership 403 (`pmcp-team-servers`), publishing 11 of 14 crates and skipping `cargo-pmcp` 0.23.0 and `pmcp-tasks` 0.1.1 as collateral; all three recovered by hand, and ownership remains a publish precondition no in-repo check covers. Phase 124's SUMMARYs and VERIFICATION were written retroactively — the work shipped 2026-08-27 but its record sat at `0/7 | Planned` for six days, which is what made every milestone-scoped GSD verb treat v2.6 as unfinished. v2.7 is now open with phase 125 already complete; previously 2026-08-22 after milestone v2.6 (AI-Package Portability) opened — pre-scoped from the v2.5 close (2026-07-27 scoping); staged requirements folded back into REQUIREMENTS.md as PKG-01..04 / PKGX-01/02 / PKGR-01; four opening decisions recorded: scope taken as scoped, Phases 122/123 stay parked contract-first, UNAS-01 (SEP-2243) deferred again unassigned, work continues on a rebased `feat/package-remote-capture-show`; previously 2026-08-22 after v2.5 milestone — **Milestone v2.5 (MCP Spec 2026-07-28 v2 Support) COMPLETE and archived**: 11 phases / 176 plans, all verification `passed`, published as pmcp v2.19.0 (PR #337, tag v2.19.0). pmcp is now a dual-version SDK — one binary serving both MCP 2025-11-25 and 2026-07-28 via per-request negotiation, v2 primary (stateless, Tasks-as-extension, JSON Schema 2020-12) and v1 cleanly severable at compile time. Nine official-suite conformance gaps closed. Closed as an override_closeout with 461 audit items recorded as open debt rather than acknowledged (two gsd-tools defects made acknowledgment unsafe — see MILESTONES.md). v2.6 (AI-Package Portability, Phases 120-124) scoped in ROADMAP.md with requirements staged in `.planning/v2.6-REQUIREMENTS-STAGED.md`, not yet opened; previously 2026-07-22 — Milestone v2.5 started: dual-version stack per the 2026-07-22 impact assessment (stateless core, Tasks-as-extension, JSON Schema 2020-12, auth SEPs, official conformance suite); v2.4 phases 106–110 moved to Validated (published as pmcp 2.17.0), v2.4 Phase 111 docs folded into v2.5 scope; previously 2026-07-19 — Phase 109 complete (Team Reference Servers: `pmcp-team-servers` one feature-flagged crate with dev-grade team-fs/mem-mcp/approval-mcp/team-mcp reference servers + additive pmcp-core namespaced `_meta` enablement + in-process "small team, one process" runtime + exportable wire-level conformance harness matching PKG-03 fixtures; TEAM-01..06 done, 7/7 must-haves re-verified after one `pmcp/http` feature-flag gap-closure fix; WR-01/WR-02 tracked as non-blocking follow-ups); previously 2026-07-18 — Phase 107 complete (Contracts & Package Format: pmcp-package adopted as workspace-excluded crate + wire-frozen via pinned-digest golden fixtures, team-server tool contracts as provable-contracts YAML; PKG-01/03 done, PKG-02 pending release-tag publish; WR-01/WR-02 review findings fixed; 3/3 must-haves verified); previously 2026-07-17 — Milestone v2.4 (Agents & Teams — SDK Extraction) started; design doc `docs/design/agents-teams-sdk-extraction-plan.md` approved incl. §6 recommendations; previously 2026-07-05 — Phase 104 complete (Task-Augmented Tool Results DX, SEP-1686 junction; ToolOutput verbatim pass-through + double-wrap tripwire + client TaskMetadata surface + migration guide; re-verified 7/7, make quality-gate green); previously 2026-06-15 — Phase 96 complete (Shape B `cargo pmcp new --kind workbook-server` scaffold + dialect-version declaration with both-lane fail-closed gate + WBEX-01 second-workbook served-schema generalization gate + WBEX-02 8-quirk corpus; WBCL-05/WBDL-02/WBEX-01/WBEX-02 validated, make quality-gate + purity-check green); previously 2026-06-14 — Phase 95 complete (pmcp-workbook-server Shape A pure-config binary mirroring pmcp-sql-server; test trio + --bundle-id proptest + reader-free purity gate + slot-9a wiring; requirement WBCL-06 validated)*
