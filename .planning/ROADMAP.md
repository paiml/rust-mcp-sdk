# Roadmap: MCP Tasks for PMCP SDK

## Milestones

- ✅ **v1.0 MCP Tasks Foundation** — Phases 1-3 (shipped 2026-02-22)
- ✅ **v1.1 Task-Prompt Bridge** — Phases 4-8 (shipped 2026-02-23)
- ✅ **v1.2 Pluggable Storage Backends** — Phases 9-13 (shipped 2026-02-24)
- ✅ **v1.3 MCP Apps Developer Experience** — Phases 14-19 (shipped 2026-02-26)
- ✅ **v1.4 Book & Course Update** — Phases 20-24 (shipped 2026-02-28)
- ✅ **v1.5 Cloud Load Testing Upload** — Phases 25-26 (shipped 2026-03-01)
- **v1.6 CLI DX Overhaul** — Phases 27-32 (in progress)
- ✅ **v1.7 SDK Maturation** — Phases 52-53 (shipped 2026-03-20)
- **v2.0 Protocol Modernization** — Phases 54-59 (in progress)
- **v2.1 rmcp Upgrades** — Phases 65-68 (in progress)
- ✅ **v2.2 Configuration-Only MCP Servers (SQL + OpenAPI toolkits)** — Phases 82-90.2 (substantially shipped)
- 🚧 **v2.3 Excel-as-Configuration MCP Servers (governed Excel CodeLanguage)** — Phases 91-96 (in progress)
- 📋 **v2.4 Agents & Teams — SDK Extraction** — Phases 106-111 (planned)
- ✅ **v2.5 MCP Spec 2026-07-28 (v2) Support** — Phases 112-119 (shipped 2026-08-22 as pmcp v2.19.0)
- 🚧 **v2.6 AI-Package Portability** — Phases 120-124 (in progress, opened 2026-08-22)
- 🚧 **v2.7 SEP-2640 Skills Conformance & Positioning** — Phase 125+ (opened 2026-09-01)

## Phases

<details>
<summary>v1.0 MCP Tasks Foundation (Phases 1-3) — SHIPPED 2026-02-22</summary>

- [x] Phase 1: Foundation Types and Store Contract (3/3 plans) — completed 2026-02-21
- [x] Phase 2: In-Memory Backend and Owner Security (3/3 plans) — completed 2026-02-22
- [x] Phase 3: Handler, Middleware, and Server Integration (3/3 plans) — completed 2026-02-22

See: `.planning/milestones/v1.0-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.1 Task-Prompt Bridge (Phases 4-8) — SHIPPED 2026-02-23</summary>

- [x] Phase 4: Foundation Types and Contracts (2/2 plans) — completed 2026-02-22
- [x] Phase 5: Partial Execution Engine (2/2 plans) — completed 2026-02-23
- [x] Phase 6: Structured Handoff and Client Continuation (2/2 plans) — completed 2026-02-23
- [x] Phase 7: Integration and End-to-End Validation (2/2 plans) — completed 2026-02-23
- [x] Phase 8: Quality Polish and Test Coverage (2/2 plans) — completed 2026-02-23

See: `.planning/milestones/v1.1-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.2 Pluggable Storage Backends (Phases 9-13) — SHIPPED 2026-02-24</summary>

- [x] Phase 9: Storage Abstraction Layer (2/2 plans) — completed 2026-02-24
- [x] Phase 10: InMemory Backend Refactor (2/2 plans) — completed 2026-02-24
- [x] Phase 11: DynamoDB Backend (2/2 plans) — completed 2026-02-24
- [x] Phase 12: Redis Backend (2/2 plans) — completed 2026-02-24
- [x] Phase 13: Feature Flag Verification (1/1 plans) — completed 2026-02-24

See: `.planning/milestones/v1.2-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.3 MCP Apps Developer Experience (Phases 14-19) — SHIPPED 2026-02-26</summary>

- [x] Phase 14: Preview Bridge Infrastructure (2/2 plans) — completed 2026-02-24
- [x] Phase 15: WASM Widget Bridge (2/2 plans) — completed 2026-02-25
- [x] Phase 16: Shared Bridge Library (2/2 plans) — completed 2026-02-26
- [x] Phase 17: Widget Authoring DX and Scaffolding (2/2 plans) — completed 2026-02-26
- [x] Phase 18: Publishing Pipeline (2/2 plans) — completed 2026-02-26
- [x] Phase 19: Ship Examples and Playwright E2E (2/2 plans) — completed 2026-02-26

See: `.planning/milestones/v1.3-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.4 Book & Course Update (Phases 20-24) — SHIPPED 2026-02-28</summary>

- [x] Phase 20: Book Load Testing (2/2 plans) — completed 2026-02-28
- [x] Phase 21: Book MCP Apps Refresh (2/2 plans) — completed 2026-02-28
- [x] Phase 22: Course Load Testing (2/2 plans) — completed 2026-02-28
- [x] Phase 23: Course MCP Apps Refresh (2/2 plans) — completed 2026-02-28
- [x] Phase 24: Course Quizzes & Exercises (2/2 plans) — completed 2026-02-28

See: `.planning/milestones/v1.4-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.5 Cloud Load Testing Upload (Phases 25-26) — SHIPPED 2026-03-01</summary>

- [x] Phase 25: Loadtest Config Upload (2/2 plans) — completed 2026-02-28
- [x] Phase 26: Add OAuth Support to Load Testing (4/4 plans) — completed 2026-03-01

See phase details in `.planning/phases/25-*` and `.planning/phases/26-*`

</details>

<details>
<summary>v1.6 CLI DX Overhaul (In Progress — paused for v2.0)</summary>

**Milestone Goal:** Normalize the cargo pmcp CLI for consistency and developer experience ahead of course recording -- fix flag inconsistencies, propagate auth to all server-facing commands, surface mcp-tester via `cargo pmcp test`, and add doctor/completions commands.

- [x] **Phase 27: Global Flag Infrastructure** - Add --no-color and --quiet as global flags available on all commands (completed 2026-03-04)
- [x] **Phase 28: Flag Normalization** - Rename and normalize all per-command flags for consistency (positional URL, --server, --verbose, --yes, -o, --format, #[arg()]) (completed 2026-03-12)
- [x] **Phase 29: Auth Flag Propagation** - Add shared OAuth and API-key flag structs to all server-facing commands (completed 2026-03-13)
- [ ] **Phase 30: Tester CLI Integration** - Surface mcp-tester subcommands through cargo pmcp test with aligned flags
- [ ] **Phase 31: New Commands** - Add cargo pmcp doctor and cargo pmcp completions commands
- [ ] **Phase 32: Help Text Polish** - Consistent help text format with descriptions and usage examples across all commands

See phase details in `.planning/phases/27-*` through `.planning/phases/32-*`

</details>

<details>
<summary>v1.7 SDK Maturation — SHIPPED 2026-03-20</summary>

**Milestone Goal:** Reduce dependency footprint and produce gap analysis against TypeScript SDK v2.

- [x] **Phase 52: Reduce transitive dependencies** - Feature-gate reqwest and tracing-subscriber, slim tokio/hyper/chrono (completed 2026-03-18)
- [x] **Phase 53: Review TypeScript SDK Updates** - Gap analysis comparing TypeScript v2 against Rust SDK (completed 2026-03-20)

See phase details in `.planning/phases/52-*` and `.planning/phases/53-*`

</details>

### v2.0 Protocol Modernization (In Progress)

**Milestone Goal:** Upgrade to MCP protocol 2025-11-25 with massive type cleanup, add Tasks with polling, Tower middleware with DNS rebinding protection, and conformance testing. Focus on streamable HTTP and stateless calls. SSE, elicitations, and notifications are de-prioritized — Tasks with status polling is the primary async pattern. This is a semver major bump enabling breaking changes for a cleaner API surface.

- [x] **Phase 54: Protocol Version 2025-11-25 + Type Cleanup** - Add all 2025-11-25 types (TaskSchema, IconSchema, AudioContent, ResourceLink), expanded capabilities, version negotiation for latest 3 versions. Breaking change: clean up legacy type aliases and deprecated fields. (completed 2026-03-20)
- [x] **Phase 54.1: Protocol Type Construction DX** - Default impls, builders, and constructors for all protocol types. Fix inconsistent construction patterns that break downstream on every upgrade. (INSERTED) (completed 2026-03-20)
- [x] **Phase 55: Tasks with Polling** - Task capability negotiation, TaskStore trait, in-memory + DynamoDB backends, task status polling via streamable HTTP. No SSE-based notifications — polling is the pattern. (completed 2026-03-21)
- [ ] **Phase 55.1: Fix MCP Tasks support** - Add execution/taskSupport to TypedTool API, wire task detection in ServerCore so standard task_store path returns CreateTaskResult instead of CallToolResult text. (INSERTED)
- [x] **Phase 56: Tower Middleware + DNS Rebinding Protection** - Tower Layer for MCP protocol concerns (host validation, DNS rebinding protection, session management, JSON-RPC routing). Axum convenience adapter. Enterprise security focus.
- [x] **Phase 57: Conformance Test Suite** - mcp-tester conformance command with core protocol, tools, resources, prompts, and tasks scenarios. Validates any MCP server against the spec. (completed 2026-03-21)
- [ ] **Phase 58: #[mcp_tool] Proc Macro** - Eliminate Box::pin(async move {}) boilerplate on every tool definition. Expand pmcp-macros crate with #[mcp_tool] attribute that accepts async fn directly, handles Arc state injection, and auto-derives input/output schema. Also addresses the foundation Arc cloning ceremony.
- [ ] **Phase 59: TypedPrompt with Auto-Deserialization** - Typed prompt equivalent of TypedToolWithOutput. Prompt arguments deserialize from HashMap<String, String> into a typed struct automatically via JsonSchema + serde, matching the tool DX pattern.

## Phase Details

<details>
<summary>Phases 27-53 (v1.6 + v1.7 — prior milestones)</summary>

### Phase 27: Global Flag Infrastructure

**Goal**: Every cargo pmcp invocation supports --no-color and --quiet for scripting and CI use
**Depends on**: Phase 26 (v1.5 complete)
**Requirements**: FLAG-08, FLAG-09
**Success Criteria** (what must be TRUE):

  1. User can pass `--no-color` to any cargo pmcp command and all terminal output is plain text (no ANSI escape codes)
  2. User can pass `--quiet` to any cargo pmcp command and only errors and explicit requested output appear
  3. Both flags work when placed before or after the subcommand (global position)

**Plans**: 2 plans
Plans:

- [ ] 27-01-PLAN.md — GlobalFlags struct, --no-color/--quiet CLI args, wire through all command dispatch, global color suppression
- [ ] 27-02-PLAN.md — Quiet mode output filtering across all commands, verbose-wins-over-quiet precedence

### Phase 28: Flag Normalization

**Goal**: Every existing cargo pmcp command uses the same conventions for URLs, server references, verbosity, confirmations, output, and format values
**Depends on**: Phase 27
**Requirements**: FLAG-01, FLAG-02, FLAG-03, FLAG-04, FLAG-05, FLAG-06, FLAG-07
**Success Criteria** (what must be TRUE):

  1. User can pass a server URL as a positional argument to any command that connects to a server (no more `--url` or `--endpoint`)
  2. User can use `--server` consistently for pmcp.run server references (no more `--server-id`)
  3. User can use `--verbose` / `-v` for detailed output on any command (no more `--detailed`)
  4. User can use `--yes` to skip confirmations and `-o` as shorthand for `--output` on any command that supports them
  5. All `--format` flags accept `text` and `json` as values (no other human-readable format names)

**Plans**: 3 plans

Plans:

- [ ] 28-01-PLAN.md — Create shared flag structs (FormatValue, OutputFlags, FormatFlags), convert deploy #[clap()] to #[arg()], clean up dead code
- [ ] 28-02-PLAN.md — Normalize test/schema/preview/connect/validate/deploy flags: URL positional, verbose removal, format normalization
- [ ] 28-03-PLAN.md — Normalize app/secret/loadtest/landing flags: URL positional, --force to --yes, -o alias, --server-id to --server

### Phase 29: Auth Flag Propagation

**Goal**: Every command that connects to an MCP server accepts OAuth and API-key authentication flags
**Depends on**: Phase 28
**Requirements**: AUTH-01, AUTH-02, AUTH-03, AUTH-04, AUTH-05, AUTH-06
**Success Criteria** (what must be TRUE):

  1. User can pass `--api-key <key>` to test check/run/generate, preview, schema export, and connect commands
  2. User can pass OAuth flags (--oauth-issuer, --oauth-client-id, --oauth-scopes, --oauth-no-cache, --oauth-redirect-port) to any of those same commands
  3. Auth flags are defined in a shared struct (AuthFlags or similar) flattened into each command, not duplicated per command
  4. Commands that already had auth support (e.g., loadtest) continue to work unchanged

**Plans**: 3 plans

Plans:

- [ ] 29-01-PLAN.md — Define AuthFlags struct, AuthMethod enum, resolve() method in flags.rs; create shared auth.rs with resolve_auth_middleware()
- [ ] 29-02-PLAN.md — Flatten AuthFlags into test check/run/generate/apps, wire handlers; migrate loadtest inline auth to shared AuthFlags
- [ ] 29-03-PLAN.md — Add AuthFlags to preview/schema export/connect; extend McpProxy with auth_header; wire connect config generation

### Phase 30: Tester CLI Integration

**Goal**: Users can run all mcp-tester capabilities through cargo pmcp test subcommands with consistent flag conventions
**Depends on**: Phase 29
**Requirements**: TEST-01, TEST-02, TEST-03, TEST-04, TEST-05, TEST-06, TEST-07, TEST-08
**Success Criteria** (what must be TRUE):

  1. User can run `cargo pmcp test compliance <url>`, `cargo pmcp test diagnose <url>`, and `cargo pmcp test compare <url1> <url2>` to validate MCP servers
  2. User can run `cargo pmcp test tools <url>`, `cargo pmcp test resources <url>`, `cargo pmcp test prompts <url>`, and `cargo pmcp test health <url>` to inspect server capabilities
  3. All `cargo pmcp test` subcommands accept the same auth flags (--api-key, OAuth) and global flags (--verbose, --no-color, --quiet) established in prior phases
  4. The standalone `mcp-tester` binary uses the same flag conventions as `cargo pmcp test` (positional URL, --verbose/-v, --yes)

**Plans:** 6 plans
Plans:

- [x] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [x] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [x] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [x] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [x] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [x] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

### Phase 31: New Commands

**Goal**: Users have workspace diagnostics and shell completion generation built into the CLI
**Depends on**: Phase 28
**Requirements**: CMD-01, CMD-02
**Success Criteria** (what must be TRUE):

  1. User can run `cargo pmcp doctor` and see validation results for workspace structure, Rust toolchain, config files, and optionally server connectivity
  2. User can run `cargo pmcp completions bash` (or zsh/fish/powershell) and pipe the output to the appropriate shell config file
  3. Both commands follow all established flag conventions (global flags, --format, help text patterns)

**Plans:** 6 plans
Plans:

- [x] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [x] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [x] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [x] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [x] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [x] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

### Phase 32: Help Text Polish

**Goal**: Every cargo pmcp command has professional, consistent help output ready for course recording
**Depends on**: Phase 31
**Requirements**: HELP-01, HELP-02
**Success Criteria** (what must be TRUE):

  1. Every command's `--help` output includes a description, grouped options (by category: connection, auth, output, etc.), and a usage examples section via `after_help`
  2. All help text follows the same structural pattern: synopsis line, categorized options, examples section
  3. Running `cargo pmcp --help` shows a clean top-level overview with all subcommands and their one-line descriptions

**Plans:** 6 plans
Plans:

- [x] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [x] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [x] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [x] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [x] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [ ] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

## Progress

**Execution Order:** Phase 27 next

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation Types | v1.0 | 3/3 | Complete | 2026-02-21 |
| 2. In-Memory Backend | v1.0 | 3/3 | Complete | 2026-02-22 |
| 3. Server Integration | v1.0 | 3/3 | Complete | 2026-02-22 |
| 4. Foundation Types | v1.1 | 2/2 | Complete | 2026-02-22 |
| 5. Execution Engine | v1.1 | 2/2 | Complete | 2026-02-23 |
| 6. Handoff + Continuation | v1.1 | 2/2 | Complete | 2026-02-23 |
| 7. Integration | v1.1 | 2/2 | Complete | 2026-02-23 |
| 8. Quality Polish | v1.1 | 2/2 | Complete | 2026-02-23 |
| 9. Storage Abstraction | v1.2 | 2/2 | Complete | 2026-02-24 |
| 10. InMemory Refactor | v1.2 | 2/2 | Complete | 2026-02-24 |
| 11. DynamoDB Backend | v1.2 | 2/2 | Complete | 2026-02-24 |
| 12. Redis Backend | v1.2 | 2/2 | Complete | 2026-02-24 |
| 13. Feature Flags | v1.2 | 1/1 | Complete | 2026-02-24 |
| 14. Preview Bridge | v1.3 | 2/2 | Complete | 2026-02-24 |
| 15. WASM Bridge | v1.3 | 2/2 | Complete | 2026-02-25 |
| 16. Shared Bridge Lib | v1.3 | 2/2 | Complete | 2026-02-26 |
| 17. Authoring DX | v1.3 | 2/2 | Complete | 2026-02-26 |
| 18. Publishing | v1.3 | 2/2 | Complete | 2026-02-26 |
| 19. Ship + E2E | v1.3 | 2/2 | Complete | 2026-02-26 |
| 20. Book Load Testing | v1.4 | 2/2 | Complete | 2026-02-28 |
| 21. Book MCP Apps | v1.4 | 2/2 | Complete | 2026-02-28 |
| 22. Course Load Testing | v1.4 | 2/2 | Complete | 2026-02-28 |
| 23. Course MCP Apps | v1.4 | 2/2 | Complete | 2026-02-28 |
| 24. Course Quizzes | v1.4 | 2/2 | Complete | 2026-02-28 |
| 25. Loadtest Upload | v1.5 | 2/2 | Complete | 2026-02-28 |
| 26. OAuth Load Testing | v1.5 | 4/4 | Complete | 2026-03-01 |
| 27. Global Flag Infrastructure | 3/3 | Complete   | 2026-03-04 | - |
| 28. Flag Normalization | 3/3 | Complete   | 2026-03-12 | - |
| 29. Auth Flag Propagation | 3/3 | Complete    | 2026-03-13 | - |
| 30. Tester CLI Integration | v1.6 | 0/? | Complete    | 2026-03-28 |
| 31. New Commands | v1.6 | 0/? | Complete    | 2026-03-28 |
| 32. Help Text Polish | v1.6 | 0/? | Complete    | 2026-03-28 |

### Phase 33: Fix mcp-tester compatibility failure

**Goal:** Bump mcp-tester to 0.2.2 and cargo-pmcp to 0.3.4, publish both to crates.io so `cargo install cargo-pmcp` works without `--locked`
**Requirements**: None (hotfix)
**Depends on:** Phase 32
**Plans:** 3/3 plans complete

Plans:

- [ ] 33-01-PLAN.md — Version bumps and crates.io publish

### Phase 34: Fix MCP Apps ChatGPT compatibility

**Goal:** Fix SDK metadata format, MIME types, and mcp-preview routes to be compatible with ChatGPT's MCP Apps implementation
**Requirements**: CHATGPT-01, CHATGPT-02, CHATGPT-03, CHATGPT-04, CHATGPT-05, CHATGPT-06
**Depends on:** Phase 33
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:

- [x] 34-01-PLAN.md — Fix tool _meta format (nested ui.resourceUri + openai/outputTemplate), add MIME type variant, dual-emit WidgetMeta
- [ ] 34-02-PLAN.md — Fix mcp-preview axum 0.8 wildcard route panic

### Phase 35: Add meta key constants module for UI/MCP Apps strings

**Goal:** Align SDK types, bridge protocol, and scaffold template with ChatGPT's official MCP Apps protocol -- add _meta to Content::Resource, fix MIME type, update bridge method names, fix scaffold
**Requirements**: P41-01, P41-02, P41-03, P41-04, P41-05
**Depends on:** Phase 34
**Plans:** 4 plans

Plans:

- [ ] TBD (run /gsd:plan-phase 35 to break down)

### Phase 36: Unify UIMimeType and ExtendedUIMimeType with From bridge

**Goal:** Add From/TryFrom conversion traits between UIMimeType and ExtendedUIMimeType so code can seamlessly convert across the feature-gate boundary
**Requirements**: MIME-BRIDGE-01
**Depends on:** Phase 35
**Plans:** 1/1 plans complete

Plans:

- [ ] 36-01-PLAN.md — TDD: From<UIMimeType> for ExtendedUIMimeType and TryFrom<ExtendedUIMimeType> for UIMimeType

### Phase 37: Add with_ui support to TypedSyncTool

**Goal:** Add with_ui() builder method to TypedSyncTool and WasmTypedTool for API parity with TypedTool, enabling sync and WASM tool authors to declare UI resource associations
**Requirements**: P37-01, P37-02, P37-03, P37-04
**Depends on:** Phase 36
**Plans:** 1/1 plans complete

Plans:

- [ ] 37-01-PLAN.md — Add ui_resource_uri field, with_ui() builder, and _meta emission to TypedSyncTool and WasmTypedTool

### Phase 38: Cache ToolInfo at registration to avoid per-request cloning

**Goal:** Cache ToolInfo and PromptInfo at builder registration time so handle_list_tools, handle_call_tool, handle_list_prompts, and task routing use cached metadata instead of calling handler.metadata() per request
**Requirements**: CACHE-01
**Depends on:** Phase 37
**Plans:** 1/1 plans complete

Plans:

- [ ] 38-01-PLAN.md — Add tool_infos/prompt_infos cache to builders, replace 6 per-request metadata() call sites with cache lookups

### Phase 39: Add deep-merge for ui meta key to prevent collision

**Goal:** Add deep_merge function for serde_json::Map and update all metadata() implementations to merge _meta instead of replacing, preventing data loss when multiple builder methods contribute to _meta. Also add with_ui() to TypedToolWithOutput and with_meta_entry() to ToolInfo.
**Requirements**: MERGE-01, MERGE-02
**Depends on:** Phase 38
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:

- [x] 39-01-PLAN.md — Add deep_merge function in ui.rs and ToolInfo::with_meta_entry builder method
- [ ] 39-02-PLAN.md — Update TypedTool, TypedSyncTool, TypedToolWithOutput, WasmTypedTool metadata() to use deep_merge; add with_ui() to TypedToolWithOutput

### Phase 40: Review ChatGPT Compatibility for Apps

**Goal:** Align SDK metadata emission with official ext-apps spec: add legacy flat key ui/resourceUri to build_meta_map, dual-emit nested ui.csp/ui.domain in WidgetMeta, add ui.visibility array format, and add ModelOnly visibility variant
**Requirements**: COMPAT-01, COMPAT-02, COMPAT-03, COMPAT-04
**Depends on:** Phase 39
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:

- [ ] 40-01-PLAN.md — Add legacy flat key ui/resourceUri to build_meta_map() for ext-apps backward compat
- [ ] 40-02-PLAN.md — Dual-emit nested ui.csp/ui.domain in WidgetMeta, add ModelOnly to ToolVisibility, emit ui.visibility array in ChatGptToolMeta

### Phase 41: ChatGPT MCP Apps Upgraded Version

**Goal:** Align SDK types, bridge protocol, and scaffold template with ChatGPT's official MCP Apps protocol -- add _meta to Content::Resource, fix MIME type, update bridge method names, fix scaffold
**Requirements**: P41-01, P41-02, P41-03, P41-04, P41-05
**Depends on:** Phase 40
**Plans:** 3/3 plans complete

Plans:

- [ ] 41-01-PLAN.md — Add _meta to Content::Resource, fix ChatGptAdapter MIME type to HtmlMcpApp
- [ ] 41-02-PLAN.md — Update bridge protocol method names in widget-runtime.mjs and index.html
- [ ] 41-03-PLAN.md — Update scaffold template with correct MIME type, with_ui(), and resource _meta

### Phase 42: Add outputSchema top level support

**Goal:** Migrate output_schema from ToolAnnotations to a top-level field on ToolInfo, aligning with MCP spec 2025-06-18. Clean break -- remove from annotations, keep pmcp:outputTypeName as codegen extension.
**Requirements**: OS-01, OS-02, OS-03, OS-04, OS-05, OS-06
**Depends on:** Phase 41
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:

- [x] 42-01-PLAN.md — Core types migration: ToolAnnotations cleanup, ToolInfo field + builder, TypedToolWithOutput rewire, macro codegen
- [ ] 42-02-PLAN.md — Consumers: cargo-pmcp schema structs, tests, example, docs update

### Phase 43: ChatGPT MCP Apps alignment

**Goal:** Fix 4 protocol gaps preventing ChatGPT from rendering MCP Apps widgets -- add _meta to ResourceInfo, filter tools/call _meta to invocation keys only, merge descriptor keys into resources/read _meta, and build URI-to-tool-meta index for auto-propagation
**Requirements**: None (hotfix-style phase)
**Depends on:** Phase 42
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:

- [ ] 43-01-PLAN.md — Add _meta field to ResourceInfo, filter with_widget_enrichment to openai/toolInvocation/*, build URI-to-tool-meta index on ServerCore, update all struct literals
- [ ] 43-02-PLAN.md — Post-process handle_list_resources and handle_read_resource to propagate tool _meta to resource responses

### Phase 44: Improving mcp-preview to support ChatGPT version

**Goal:** Add --mode chatgpt flag to mcp-preview enabling strict ChatGPT protocol validation, postMessage emulation with window.openai stub, and a Protocol diagnostics tab in DevTools
**Requirements**: P44-MODE, P44-CONFIG, P44-RESOURCEMETA, P44-PROTOCOL-TAB, P44-CHATGPT-EMULATION, P44-BADGE
**Depends on:** Phase 43
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:

- [x] 44-01-PLAN.md — Rust-side mode plumbing: PreviewMode enum, CLI --mode flag, ConfigResponse with keys, ResourceInfo _meta, banner
- [x] 44-02-PLAN.md — Browser-side Protocol tab, ChatGPT postMessage emulation, window.openai stub, mode badge

### Phase 45: Extend MCP Apps Support to Claude Desktop

**Goal:** Refactor SDK metadata emission to standard-only default with opt-in host layers, normalize widget-runtime bridge with extensions namespace, and update mcp-preview standard mode -- enabling Claude Desktop and all standard MCP Apps hosts to work without ChatGPT-specific keys
**Requirements**: P45-STANDARD-DEFAULT, P45-HOST-LAYER, P45-URI-INDEX, P45-BRIDGE-NORMALIZE, P45-EXTENSIONS-NS, P45-PREVIEW-STANDARD, P45-EXAMPLES-VERIFY
**Depends on:** Phase 44
**Plans:** 3/3 plans complete

Plans:

- [ ] 45-01-PLAN.md — Refactor metadata emission to standard-only default + host layer enrichment pipeline on ServerCoreBuilder
- [ ] 45-02-PLAN.md — Normalize widget-runtime bridge with extensions namespace for ChatGPT-specific APIs
- [ ] 45-03-PLAN.md — Update mcp-preview standard mode default + verify examples render in both modes

### Phase 46: MCP Bridge Review and Fixes

**Goal:** Fix the mcpBridge data delivery pipeline so widgets receive structuredContent from tool responses across all MCP hosts, add method name normalization for cross-host compatibility, replace fragile setTimeout delivery with readiness signals, and add Bridge diagnostics tab to mcp-preview
**Requirements**: BRIDGE-01, BRIDGE-02, BRIDGE-03, BRIDGE-04, BRIDGE-05, BRIDGE-06, BRIDGE-07, BRIDGE-08
**Depends on:** Phase 45
**Success Criteria** (what must be TRUE):

  1. Widgets receive tool result data regardless of whether the host sends short-form (ui/toolResult) or long-form (ui/notifications/tool-result) method names
  2. McpApps adapter bridge provides onToolResult callback API on mcpBridge
  3. mcp-preview waits for widget readiness signal before delivering tool results (no setTimeout)
  4. Bridge diagnostics tab in mcp-preview shows PostMessage traffic log, handshake trace, and current mode

**Plans:** 2/3 plans executed

Plans:

- [ ] 46-01-PLAN.md — Fix bridge protocol method name mismatch in adapter.rs and App class normalization
- [ ] 46-02-PLAN.md — Fix mcp-preview tool result delivery with readiness signal and dual method emission
- [ ] 46-03-PLAN.md — Add Bridge diagnostics tab to mcp-preview and verify complete fix with real widget

### Phase 47: Add MCP App support to mcp-tester

**Goal:** Add MCP App protocol metadata validation to mcp-tester and cargo pmcp test, enabling CLI-based App compliance checks (metadata-only, no browser) with standard and host-specific modes
**Requirements**: APP-VAL-01, APP-VAL-02, APP-VAL-03, APP-VAL-04, APP-VAL-05
**Depends on:** Phase 46
**Success Criteria** (what must be TRUE):

  1. User can run `mcp-tester apps <url>` or `cargo pmcp test apps --url <url>` to validate App metadata on any MCP server
  2. Validation checks ui.resourceUri, MIME types, resource cross-references, and optionally ChatGPT-specific keys
  3. `cargo pmcp test check` shows hint when App-capable tools are detected
  4. --strict promotes warnings to failures, --tool filters to single tool, --mode selects host-specific checks

**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:

- [ ] 47-01-PLAN.md -- AppValidator module, TestCategory::Apps, mcp-tester apps subcommand
- [ ] 47-02-PLAN.md -- cargo pmcp test apps subcommand, check command App hint

### Phase 48: MCP Apps Documentation and Education Refresh

**Goal:** Update all documentation, tooling READMEs, book chapters, and course materials to reflect the current MCP Apps capabilities including multi-host support (ChatGPT, Claude Desktop), mcp-tester apps validation, mcp-preview improvements, and the developer guide. Also fix mcp-preview theme support by sending CSS variable palettes in host context.
**Requirements**: DOCS-01, DOCS-02, DOCS-03, DOCS-04, PREVIEW-01
**Depends on:** Phase 47
**Success Criteria** (what must be TRUE):

  1. mcp-tester README documents the `apps` subcommand with usage examples and validation modes
  2. mcp-preview README describes current capabilities including multi-host preview, widget runtime, and DevTools
  3. pmcp-book MCP Apps chapters are updated with current tooling, host layer system, and developer guide content
  4. pmcp-course materials are aligned with book updates
  5. mcp-preview sends `styles.variables` CSS custom properties in host context so widgets respond to theme changes

**Plans:** 3/3 plans complete

Plans:

- [ ] 48-01-PLAN.md — Update mcp-tester/mcp-preview READMEs and rewrite book ch12-5 MCP Apps chapter with GUIDE.md content
- [ ] 48-02-PLAN.md — Update pmcp-course ch20 MCP Apps chapters and ch11-02 mcp-tester lesson to align with book
- [ ] 48-03-PLAN.md — Add theme CSS variable palettes to mcp-preview host context for ext-apps widget theming

### Phase 49: Bump dependencies (reqwest 0.13, jsonschema 0.45)

**Goal:** Upgrade reqwest from 0.12 to 0.13 and jsonschema from 0.38 to 0.45 across the workspace, updating feature flags, MSRV, deprecated methods, and template strings
**Requirements**: DEP-01
**Depends on:** Phase 48
**Success Criteria** (what must be TRUE):

  1. All four workspace Cargo.toml files reference reqwest 0.13 with correct feature names (rustls, form)
  2. jsonschema bumped to 0.45 with MSRV raised to 1.83.0
  3. Template strings in deploy/scaffold generate correct reqwest 0.13 lines for new projects
  4. `make quality-gate` passes with zero warnings

**Plans:** 1/1 plans complete

Plans:

- [ ] 49-01-PLAN.md — Update all Cargo.toml files, MSRV, deprecated methods, and template strings for reqwest 0.13 + jsonschema 0.45

### Phase 50: Improve Binary Release

**Goal:** Fix the broken binary release auto-trigger, add Apple Silicon and Linux ARM64 targets, create installer scripts, add cargo-binstall metadata, and generate SHA256 checksums for mcp-tester and mcp-preview
**Requirements**: TRIGGER, ARM-MAC, ARM-LIN, CHECKSUMS, INSTALL-SH, INSTALL-PS1, BINSTALL
**Depends on:** Phase 49
**Success Criteria** (what must be TRUE):

  1. Pushing a v* tag triggers binary builds for both mcp-tester and mcp-preview automatically
  2. Release includes binaries for 5 targets: x86_64-linux, aarch64-linux, x86_64-macos, aarch64-macos, x86_64-windows
  3. Each binary has a corresponding SHA256 checksum file on the release
  4. Users can install binaries via curl|sh (Linux/macOS) or PowerShell (Windows)
  5. cargo binstall metadata is present in both crate Cargo.toml files

**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:

- [ ] 50-01-PLAN.md — Convert binary workflows to reusable workflow_call, fix runner labels, add ARM64 targets, add SHA256 checksums
- [ ] 50-02-PLAN.md — Create install.sh and install.ps1 installer scripts, add cargo-binstall metadata to Cargo.toml files

### Phase 51: PMCP MCP Server

**Goal:** Build a developer tools MCP server (crates/pmcp-server/) that provides protocol testing, scaffolding, schema export, documentation resources, and guided workflow prompts over streamable HTTP -- deployed at pmcp.run and released as cross-platform binary
**Requirements**: None (new feature)
**Depends on:** Phase 50
**Success Criteria** (what must be TRUE):

  1. Server binary starts and serves 5 tools (test_check, test_generate, test_apps, scaffold, schema_export) over streamable HTTP
  2. Server provides 9 documentation resources via pmcp:// URIs with embedded markdown content
  3. Server provides 7 guided workflow prompts (quickstart, create-mcp-server, add-tool, diagnose, setup-auth, debug-protocol-error, migrate)
  4. All content is statically embedded in the binary via include_str! -- no runtime file dependencies
  5. Release workflow builds pmcp-server binaries for 5 platform targets and publishes to crates.io

**Plans:** 5/5 plans complete

Plans:

- [ ] 51-01-PLAN.md — Crate scaffold, workspace integration, server skeleton, ScenarioGenerator API addition
- [ ] 51-02-PLAN.md — Testing tools: test_check, test_generate, test_apps wrapping mcp-tester library
- [ ] 51-03-PLAN.md — Build tools: scaffold (code templates) and schema_export (schema discovery)
- [ ] 51-04-PLAN.md — Embedded content, documentation resources handler, workflow prompt handlers
- [ ] 51-05-PLAN.md — Wire all tools/resources/prompts into server builder, CI workflow updates

### Phase 52: Reduce transitive dependencies

**Goal:** Reduce pmcp crate's transitive dependency count from ~249 to ~150-185 by removing unused deps, slimming feature flags, making reqwest optional behind `http-client` feature, and making tracing-subscriber optional behind `logging` feature
**Requirements**: DEP-REDUCE-01, DEP-REDUCE-02, DEP-REDUCE-03, DEP-REDUCE-04, DEP-REDUCE-05, DEP-REDUCE-06, DEP-REDUCE-07
**Depends on:** Phase 51
**Plans:** 2 plans — Complete (2026-03-18)

Plans:

- [x] 52-01-PLAN.md — Cargo.toml: remove unused deps, slim features, make reqwest/tracing-subscriber optional
- [x] 52-02-PLAN.md — Source code: cfg gates for optional deps, full feature matrix verification

### Phase 53: Review TypeScript SDK Updates

**Goal:** Compare TypeScript MCP SDK v2 against Rust SDK v1.20.0 to identify gaps worth adopting. Produce gap analysis with prioritized recommendations covering protocol negotiation, conformance testing, MCP Apps, Tasks, and framework adapters.
**Requirements**: GAP-ANALYSIS
**Depends on:** Phase 52
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:

- [x] 53-01-PLAN.md — Deep verification of TypeScript vs Rust SDK source differences across 6 domains
- [x] 53-02-PLAN.md — Gap analysis report with prioritized recommendations and proposed implementation phases

</details>

### Phase 54: Protocol Version 2025-11-25 + Type Cleanup

**Goal:** Upgrade Rust SDK to MCP protocol 2025-11-25 with version negotiation (latest 3 versions). Add 20+ new types (TaskSchema, IconSchema, AudioContent, ResourceLink, expanded ServerCapabilities/ClientCapabilities). Clean up legacy type aliases and deprecated fields. Breaking change — part of the v2.0.0 semver bump.
**Requirements**: PROTO-2025-11-25, VERSION-NEGOTIATION, TYPE-CLEANUP
**Depends on:** Phase 53
**Plans:** 4/4 plans complete

Plans:

- [x] 54-01-PLAN.md — Module split (protocol.rs -> 7 domain sub-modules) + version negotiation update to 2025-11-25
- [ ] 54-02-PLAN.md — Add 33 new types (task, content, sampling, elicitation, capabilities) + fix IncludeContext, LogLevel bugs
- [ ] 54-03-PLAN.md — Fix internal src/ imports, remove 11 legacy type aliases
- [ ] 54-04-PLAN.md — Fix external imports (examples/tests/workspace), write MIGRATION.md

### Phase 54.1: Protocol Type Construction DX (INSERTED)

**Goal:** Add Default impls, builder methods, and constructors for all protocol types so downstream users can construct types without specifying every Optional field. Fix the inconsistency where some types have constructors, some don't, and enum variants have neither. Prevents painful migration breaks when new fields are added.
**Requirements**: PROTO-TYPE-DX
**Depends on:** Phase 54
**Plans:** 3/3 plans complete

Plans:

- [x] 54.1-01-PLAN.md — Add constructors/Default/#[non_exhaustive]/.with_*() to resources.rs, prompts.rs, content.rs (Content enum helpers)
- [x] 54.1-02-PLAN.md — Add constructors/Default/#[non_exhaustive]/.with_*() to protocol/mod.rs, tasks.rs, sampling.rs, notifications.rs, capabilities.rs, tools.rs
- [x] 54.1-03-PLAN.md — Migrate all external consumers (src/, tests/, examples/, workspace crates) to constructors, update MIGRATION.md

### Phase 55: Tasks with Polling

**Goal:** Reconcile SDK task types as canonical source, add TaskStore trait with InMemoryTaskStore to SDK, wire into server builder and request dispatch with ServerCapabilities.tasks capability negotiation. Polling-only async pattern -- no SSE notifications.
**Requirements**: TASKS-POLLING, TASK-STORE, TASK-CAPABILITIES
**Depends on:** Phase 54.1
**Success Criteria** (what must be TRUE):

  1. SDK TaskStatus has is_terminal() and can_transition_to() utility methods matching pmcp-tasks
  2. Task.ttl serializes as null (not omitted) when None, per MCP spec
  3. SDK defines TaskStore trait with create/get/list/cancel/update_status/cleanup_expired
  4. InMemoryTaskStore provides dev/test implementation with owner isolation, state machine, TTL
  5. Builder.task_store() registers Arc<dyn TaskStore> and auto-configures ServerCapabilities.tasks
  6. Server dispatches tasks/get, tasks/list, tasks/cancel through TaskStore

**Plans:** 3/3 plans executed

Plans:

- [x] 55-01-PLAN.md — SDK task type reconciliation: add utility methods, fix TTL serialization
- [x] 55-02-PLAN.md — TaskStore trait + InMemoryTaskStore in SDK core
- [x] 55-03-PLAN.md — Server builder integration, core dispatch, capability negotiation, re-exports

### Phase 55.1: Fix MCP Tasks support (INSERTED)

**Goal:** Fix three SDK-side gaps that prevent the standard task_store path from returning proper CreateTaskResult wire format. Add execution/taskSupport to all TypedTool variants, wire task detection in ServerCore handle_call_tool, and add _meta with io.modelcontextprotocol/related-task to CreateTaskResult responses.
**Requirements**: D-01, D-02, D-03, D-04, D-05, D-06, D-07, D-08, D-09
**Depends on:** Phase 55
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:

- [x] 55.1-01-PLAN.md — Add execution field and with_execution() to all TypedTool variants
- [x] 55.1-02-PLAN.md — Wire task detection in core.rs, return CreateTaskResult with _meta

### Phase 56: Tower Middleware + DNS Rebinding Protection

**Goal:** Build a Tower Layer stack for MCP server hosting: DNS rebinding protection (Host + Origin header validation against allowed origins), security response headers, and origin-locked CORS. Axum convenience adapter (`pmcp::axum::router()`) for the 90% case. Enterprise security focus -- fix CVE-pattern wildcard CORS and achieve MCP spec 2025-03-26 Origin validation compliance.
**Requirements**: TOWER-MIDDLEWARE, DNS-REBINDING, AXUM-ADAPTER
**Depends on:** Phase 54
**Success Criteria** (what must be TRUE):

  1. DnsRebindingLayer validates Host header (always) and Origin header (when present), returns 403 on mismatch
  2. SecurityHeadersLayer adds X-Content-Type-Options: nosniff, X-Frame-Options: DENY, Cache-Control: no-store
  3. `pmcp::axum::router(server)` returns axum::Router with DNS rebinding + security headers + origin-locked CORS
  4. StreamableHttpServer no longer uses wildcard `Access-Control-Allow-Origin: *`
  5. Example 55 (ServerHttpMiddleware) still compiles unchanged

**Plans:** 3/3 plans complete

Plans:

- [x] 56-01-PLAN.md -- Tower deps, AllowedOrigins config, DnsRebindingLayer, SecurityHeadersLayer with unit tests (completed 2026-03-21)
- [x] 56-02-PLAN.md -- Axum router convenience function, StreamableHttpServer CORS fix, lib.rs re-exports (completed 2026-03-21)
- [ ] 56-03-PLAN.md -- Gap closure: apply Tower layers in StreamableHttpServer::start(), delete add_cors_headers, pre-resolve AllowedOrigins in ServerState

### Phase 57: Conformance Test Suite

**Goal:** Add `mcp-tester conformance <url>` command that validates any MCP server against the protocol spec. Core scenarios: initialize handshake, tools CRUD, resources CRUD, prompts CRUD, task lifecycle. Modeled after TypeScript SDK's @modelcontextprotocol/conformance infrastructure.
**Requirements**: CONFORMANCE-CLI, CONFORMANCE-SCENARIOS
**Depends on:** Phase 55
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:

- [x] 57-01-PLAN.md — Conformance module with ConformanceRunner orchestrator and 5 domain scenario groups (Core, Tools, Resources, Prompts, Tasks) (completed 2026-03-21)
- [x] 57-02-PLAN.md — CLI integration: replace Compliance with Conformance in mcp-tester, add cargo pmcp test conformance (completed 2026-03-21)

### Phase 58: #[mcp_tool] Proc Macro

**Goal:** Expand pmcp-macros crate with `#[mcp_tool]` attribute macro that eliminates `Box::pin(async move {})` boilerplate on tool definitions. Accepts `async fn(input: T, extra: RequestHandlerExtra) -> Result<Output>` directly. Handles Arc state injection for composition scenarios (eliminates the foundation cloning ceremony). Auto-derives input/output JSON schema from types.
**Requirements**: TOOL-MACRO, STATE-INJECTION
**Depends on:** Phase 54
**Plans:** 3/3 plans complete

Plans:

- [x] 58-01-PLAN.md — State<T> type, parameter classification, standalone #[mcp_tool] macro
- [x] 58-02-PLAN.md — #[mcp_server] impl-block macro with McpServer trait and builder extension
- [x] 58-03-PLAN.md — Integration tests, compile-fail tests, and example 63

### Phase 59: TypedPrompt with Auto-Deserialization

**Goal:** Add `TypedPrompt` analogous to `TypedToolWithOutput` for prompts. Prompt arguments deserialize from `HashMap<String, String>` into a typed struct via JsonSchema + serde, eliminating the manual `args.get("x").ok_or()?.parse()?` pattern on every prompt. Builder-friendly registration via `.prompt("name", TypedPrompt::new(handler))`.
**Requirements**: TYPED-PROMPT, PROMPT-SCHEMA
**Depends on:** Phase 54
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:

- [x] 59-01-PLAN.md — TypedPrompt runtime type and standalone #[mcp_prompt] attribute macro
- [x] 59-02-PLAN.md — #[mcp_server] prompt extension, integration tests, compile-fail tests, and example 64

### Phase 60: Clean up mcp-preview side tabs

**Goal:** Clean up the mcp-preview DevTools side panel: remove the Console tab, make the panel resizable and collapsible with a draggable left boundary and header toggle button, and add a global Clear All button.
**Requirements**: D-01, D-02, D-03, D-04, D-05, D-06, D-07, D-08, D-09, D-10, D-11, D-12, D-13
**Depends on:** Phase 59
**Plans:** 1/1 plans complete

Plans:

- [x] 60-01-PLAN.md — Remove Console tab, add resizable/collapsible panel with toggle button and global Clear All

### Phase 61: Add OAuth support to mcp-preview

**Goal:** Add browser-based OAuth PKCE authentication to mcp-preview so developers can test MCP Apps against OAuth-protected servers on pmcp.run, with dynamic auth header updates, login modal, and CLI flag wiring.
**Requirements**: TBD
**Depends on:** Phase 60
**Plans:** 3/3 plans complete

Plans:

- [x] 61-01-PLAN.md -- Server-side OAuth infrastructure (RwLock proxy, auth handlers, callback page, config exposure, 401/403 propagation)
- [x] 61-02-PLAN.md -- Browser-side OAuth popup flow (OAuthManager, PKCE, login modal) and CLI OAuth flag wiring
- [x] 61-03-PLAN.md -- Gap closure: fix forward_raw/forward_mcp 401/403 propagation for WASM bridge path

### Phase 62: mcp-pen-test

**Goal:** Add automated penetration testing for MCP server endpoints via `cargo pmcp pentest <url>` -- probes for prompt injection, tool poisoning, and session security vulnerabilities with severity classification, rate limiting, and SARIF output for CI integration.
**Requirements**: None (new feature, not tracked in REQUIREMENTS.md)
**Depends on:** Phase 61
**Plans:** 3/3 plans complete

Plans:

- [x] 62-01-PLAN.md -- Foundation: types, config, rate limiter, report (JSON/SARIF), discovery, payload library, CLI command skeleton
- [ ] 62-02-PLAN.md -- Prompt injection (PI-01..PI-07) and tool poisoning (TP-01..TP-06) attack runners
- [x] 62-03-PLAN.md -- Session security (SS-01..SS-06) attack runner and final integration verification

### Phase 63: advanced-pentest-attack-modules

**Goal:** Extend pentest with 4 new attack categories (transport, auth, data exfiltration, protocol abuse), --profile quick/deep flag, and deep fuzzing mutations -- 13 new attacks (TR-01..03, AF-01..03, DE-01..03, PA-01..04) across 32 total attack IDs.
**Requirements**: None (new feature, not tracked in REQUIREMENTS.md)
**Depends on:** Phase 62
**Plans:** 3/3 plans complete

Plans:

- [x] 63-01-PLAN.md -- Foundation: extend AttackCategory enum, add PentestProfile, --profile flag, 4 attack module stubs, SARIF rules, engine dispatch
- [ ] 63-02-PLAN.md -- Transport security (TR-01..03) and auth flow (AF-01..03) attack runners
- [x] 63-03-PLAN.md -- Data exfiltration (DE-01..03), protocol abuse (PA-01..04) attack runners, deep fuzzing mode

### Phase 64: secrets-deployment-integration

**Goal:** Wire `cargo pmcp secret` into deployment targets so secrets are injected as environment variables at deploy time. Five workstreams: (1) AWS Lambda — resolve secrets from configured provider and inject as Lambda env vars in CDK context during `cargo pmcp deploy --target aws-lambda`. (2) pmcp.run — ensure `cargo pmcp secret set --server <id>` sends server ID for backend-side env var trigger, and `cargo pmcp deploy --target pmcp-run` transmits secret requirements to the backend. (3) SDK support — add thin `pmcp::secrets` module with `get`/`require` helpers that read env vars with helpful error messages pointing to `cargo pmcp secret set`. (4) Local dev — `cargo pmcp dev` reads local secrets and sets them as env vars for the child server process. (5) Documentation — update cargo-pmcp README, secret command help text, deployment docs, and add SDK-level rustdoc examples.
**Requirements**: D-01 through D-17 (from CONTEXT.md)
**Depends on:** Phase 63
**Plans:** 3/3 plans complete

Plans:

- [x] 64-01-PLAN.md -- Secret resolution logic + deploy pipeline integration (dotenvy, resolve_secrets, CDK env passthrough)
- [x] 64-02-PLAN.md -- SDK pmcp::secrets thin reader module (get/require helpers, SecretError)
- [ ] 64-03-PLAN.md -- Dev command .env loading + documentation (dev.rs injection, README, CLI help)

### v2.1 rmcp Upgrades (In Progress)

**Milestone Goal:** Close the credibility and developer-experience gaps where the official Rust MCP SDK (rmcp) outshines PMCP -- documentation accuracy, feature gate presentation, macro documentation, example index, and repo hygiene. No new runtime dependencies; all fixes are configuration changes, file rewrites, and targeted attribute additions.

- [x] **Phase 65: Examples Cleanup and Protocol Accuracy** - Replace broken examples/README.md, fix protocol badge, resolve 17 orphan example files and 4 duplicate number prefixes (completed 2026-04-10)
- [x] **Phase 66: Macros Documentation Rewrite** - Rewrite pmcp-macros README to document current #[mcp_tool]/#[mcp_server]/#[mcp_prompt]/#[mcp_resource] API with migration guide (completed 2026-04-11)
- [x] **Phase 67: docs.rs Pipeline and Feature Flags** - Enable doc_auto_cfg for automatic feature badges, explicit feature list in docs.rs metadata, feature flag table, zero rustdoc warnings (completed 2026-04-12)
- [ ] **Phase 68: General Documentation Polish** - Update lib.rs doctests to TypedToolWithOutput pattern, add transport matrix, CI enforcement gates for drift prevention

## Phase Details — v2.1 Milestone

### Phase 65: Examples Cleanup and Protocol Accuracy

**Goal**: Developers browsing the examples/ directory and README see accurate PMCP content with correct protocol version, every example file is runnable, and no numbering collisions exist
**Depends on**: Phase 64
**Requirements**: EXMP-01, EXMP-02, EXMP-03, PROT-01
**Success Criteria** (what must be TRUE):

  1. `examples/README.md` contains a PMCP example index organized by category (transport, tools, resources, prompts, tasks, apps) with required features and run commands for each example
  2. Every `.rs` file in `examples/` has a corresponding `[[example]]` entry in `Cargo.toml` with correct `required-features`, and `cargo run --example <name>` works for each
  3. No two example files share the same numbered prefix -- `ls examples/*.rs | awk -F_ '{print $1}' | sort | uniq -d` returns empty
  4. The README.md MCP-Compatible badge and compatibility table display protocol version `2025-11-25`, matching `LATEST_PROTOCOL_VERSION` in source code

**Plans:** 3/3 plans complete
Plans:

- [x] 65-01-PLAN.md — Audit orphan examples + fix protocol badge (EXMP-02, PROT-01)
- [x] 65-02-PLAN.md — Renumber all examples with role-prefix scheme (EXMP-03)
- [x] 65-03-PLAN.md — Write examples/README.md index (EXMP-01)

### Phase 66: Macros Documentation Rewrite

**Goal**: A developer reading pmcp-macros documentation (on docs.rs or GitHub) sees accurate documentation of #[mcp_tool], #[mcp_server], #[mcp_prompt], and #[mcp_resource] as the primary API, with a clear migration path from deprecated macros
**Depends on**: Phase 65
**Requirements**: MACR-01, MACR-02, MACR-03
**Success Criteria** (what must be TRUE):

  1. `pmcp-macros/README.md` documents `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, and `#[mcp_resource]` as the primary API with working code examples that compile
  2. A migration section guides users from deprecated `#[tool]`/`#[tool_router]` to `#[mcp_tool]`/`#[mcp_server]` with before/after code comparisons
  3. `pmcp-macros/src/lib.rs` uses `include_str!("../README.md")` so that `docs.rs/pmcp-macros` renders the rewritten README as the crate-level documentation
  4. No references to stale version numbers (e.g., `pmcp = { version = "1.*" }`) appear in the macros README

**Plans:** 6 plans
Plans:

- [x] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [x] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [x] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [x] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [x] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [ ] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

### Phase 67: docs.rs Pipeline and Feature Flags

**Goal**: docs.rs renders PMCP with automatic feature badges on all feature-gated items, an explicit feature list preventing internal APIs from surfacing, a documented feature flag table, and zero rustdoc warnings
**Depends on**: Phase 66
**Requirements**: DRSD-01, DRSD-02, DRSD-03, DRSD-04
**Success Criteria** (what must be TRUE):

  1. `src/lib.rs` contains `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` and all ~145 feature-gated items on docs.rs display automatic feature availability badges
  2. `Cargo.toml` `[package.metadata.docs.rs]` uses an explicit feature list (~13 user-facing features) instead of `all-features = true`, preventing test helpers and internal features from surfacing
  3. A feature flag table in `lib.rs` doc comments documents all user-facing features with descriptions and what they enable
  4. `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` exits with zero warnings -- all broken intra-doc links and unclosed HTML tags resolved
  5. CI includes a `make doc-check` target that enforces zero rustdoc warnings on every PR

**Plans:** 6 plans
Plans:

- [x] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [x] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [x] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [ ] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [ ] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [ ] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

### Phase 67.1: Code Mode Support (INSERTED)

**Goal:** External MCP server developers can add Code Mode (validate → approve → execute) to their servers using PMCP SDK crates, with a `#[derive(CodeMode)]` proc macro, pluggable `PolicyEvaluator` + `CodeExecutor` traits, zeroizing token secrets, and a complete worked example — unblocking an imminent MCP server launch that depends on this capability.
**Depends on:** Phase 67
**Requirements**: CMSUP-01, CMSUP-02, CMSUP-03, CMSUP-04, CMSUP-05, CMSUP-06
**Success Criteria** (what must be TRUE):

  1. `crates/pmcp-code-mode/` exists in the rust-mcp-sdk workspace containing the moved + hardened Code Mode core (validation pipeline, `PolicyEvaluator`, `CedarPolicyEvaluator`, `NoopPolicyEvaluator`, new `CodeExecutor` trait, new `TokenSecret` newtype with zeroization) and all existing tests pass
  2. `crates/pmcp-code-mode-derive/` exists and provides a working `#[derive(CodeMode)]` proc macro that emits a `register_code_mode_tools(builder)` method, enforces `Send + Sync` at compile time, and has `trybuild` compile-pass + compile-fail snapshot coverage
  3. `pmcp-code-mode/src/lib.rs` re-exports `async_trait` (`pub use async_trait::async_trait;`) and generated derive output uses `#[pmcp_code_mode::async_trait]` to avoid version conflicts
  4. A complete worked example in `examples/` (e.g. `XX_code_mode_graphql.rs`) demonstrates: struct annotation → `register_code_mode_tools` → `validate_code` → approval token → `execute_code` round trip using `NoopPolicyEvaluator`
  5. Contract YAMLs for `pmcp-code-mode` and `pmcp-code-mode-derive` exist under `../provable-contracts/contracts/` and `pmat comply check` passes on both
  6. `make quality-gate` passes workspace-wide (zero clippy warnings, zero SATD, all tests green, format clean) and both new crates are positioned in the publishing order documented in CLAUDE.md, ready for the next release phase

**Plans:** 6/6 plans complete

Plans:

- [x] 67.1-01-PLAN.md — Crate scaffolding + source move into workspace
- [x] 67.1-02-PLAN.md — Security hardening (TokenSecret, NoopPolicyEvaluator, async_trait re-export)
- [x] 67.1-03-PLAN.md — CodeExecutor high-level trait
- [x] 67.1-04-PLAN.md — pmcp-code-mode-derive proc macro (#[derive(CodeMode)] + trybuild)
- [x] 67.1-05-PLAN.md — Property tests + fuzz targets
- [x] 67.1-06-PLAN.md — End-to-end example + CRATE-READMEs + SECURITY.md + quality-gate

### Phase 67.2: Code Mode Derive Hardening (INSERTED)

**Goal:** Fix critical derive macro issues from pmcp.run team review (policy_evaluator not called, static ValidationContext, hardcoded "graphql"), address review warnings, and resolve high-priority performance/quality issues from IMPROVEMENTS.md.
**Depends on:** Phase 67.1
**Requirements**: CMSUP-07, CMSUP-08, CMSUP-09, CMSUP-10
**Success Criteria** (what must be TRUE):
  **Derive macro — critical (pmcp.run review items 1-3):**

  1. Generated `ValidateCodeHandler` calls `policy_evaluator.evaluate_operation()` between pipeline validation and token generation — the security contract is enforced
  2. `#[code_mode(context_from = "method_name")]` attribute extracts real `ValidationContext` from `RequestHandlerExtra` or a struct method, replacing hardcoded placeholders
  3. `#[code_mode(language = "graphql"|"javascript"|"sql")]` attribute parameterizes tool metadata — SQL/OpenAPI servers get correct tool schemas
  **Derive macro — warnings (pmcp.run review items 4-8):**

  4. `HmacTokenGenerator::new` returns `Result` instead of panicking on short secrets
  5. Trybuild compile-fail tests cover `token_secret` and `code_executor` absent fields
  6. Generated handlers share a single `Arc<ValidationPipeline>` instead of constructing two
  **Performance (IMPROVEMENTS.md P-01, P-03):**

  7. eval.rs array methods use scope-chain/push-pop instead of cloning entire HashMap per element (P-01)
  8. Async GraphQL validation fallback reuses parsed `query_info` instead of re-parsing (P-03)
  *Deferred: P-02 (double SWC parse) requires new `ValidatedCode` type threading AST across javascript.rs/executor.rs — deferred to a future phase*
  **Code quality (IMPROVEMENTS.md Q-01 through Q-04, R-01):**

  9. `json_to_string` / `value_to_string` unified into one function (Q-01)
  10. `LoopContinue`/`LoopBreak` moved to internal `StepOutcome` enum, removed from public `ExecutionError` (Q-04)
  11. `ValidationResponse` wraps `ValidationResult` instead of duplicating all fields (R-01)
  **Baseline:**

  12. All existing tests pass, `cargo test -p pmcp-code-mode -p pmcp-code-mode-derive` green
  13. Clippy suppressions reduced (trivially fixable: `useless_format`, `derivable_impls`, etc.)

**Plans:** 6/6 plans complete
Plans:

- [x] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [x] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [ ] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [ ] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [ ] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [ ] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

### Phase 68: General Documentation Polish

**Goal**: Crate-level documentation showcases current best practices (TypedToolWithOutput, proc macros), transport types are discoverable, and CI gates prevent future documentation drift
**Depends on**: Phase 67
**Requirements**: PLSH-01, PLSH-02, PLSH-03
**Success Criteria** (what must be TRUE):

  1. `lib.rs` crate-level doc examples compile and demonstrate the `TypedToolWithOutput` pattern and current builder APIs (not legacy `Server::builder()` or `ToolHandler`)
  2. A transport matrix table in `lib.rs` doc comments lists all supported transports (stdio, streamable HTTP, SSE) with links to their actual module/type paths
  3. CI enforces that the count of `[[example]]` entries in `Cargo.toml` matches the count of `.rs` files in `examples/`, failing the build on mismatch
  4. `cargo semver-checks check-release` runs in CI on every PR to prevent accidental API breakage during documentation changes

**Plans:** 6 plans
Plans:

- [ ] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [ ] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [ ] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [ ] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [ ] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [ ] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

## Progress — v2.1 Milestone

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 65. Examples Cleanup + Protocol Accuracy | v2.1 | 3/3 | Complete    | 2026-04-10 |
| 66. Macros Documentation Rewrite | v2.1 | 5/5 | Complete    | 2026-04-11 |
| 67. docs.rs Pipeline + Feature Flags | v2.1 | 6/6 | Complete    | 2026-04-12 |
| 68. General Documentation Polish | v2.1 | 0/? | Not started | - |

## Backlog

Parking lot for unsequenced ideas. Items here aren't scheduled — promote with `/gsd:review-backlog` when ready.

### Phase 999.1: Delete DEFAULT_PROTOCOL_VERSION constant and make callsites explicit (BACKLOG)

**Goal:** Remove the public `DEFAULT_PROTOCOL_VERSION` re-export and replace each of its ~15 callsites with an explicit choice — either `LATEST_PROTOCOL_VERSION` (where the code is advertising what this SDK supports) or a literal `"2025-03-26"` with a `// backward-compat fallback` comment (where the code genuinely wants the widest-compatible version for un-negotiated peers). The name `DEFAULT` is misleading: nothing is actually "default" about it — it's a specific compat choice that happens to be older than `LATEST`, and that distinction is invisible at every callsite.

**Scope:**

- Breaking API change (public re-export at `src/lib.rs:307` + `src/types/mod.rs:32`) — requires minor version bump and release note
- Callsites to audit and convert:
  - `src/types/protocol/mod.rs:32` — `impl Default for ProtocolVersion`
  - `src/server/streamable_http_server.rs` lines 560, 971, 979, 981, 985, 1225, 1231, 1233, 1236
  - `src/server/core.rs:1329` and `:1376`
  - `src/shared/event_store.rs:367`
  - `src/lib.rs:286` — public doctest asserting the value
  - `src/types/protocol/version.rs:51,65` — unit tests
  - `benches/comprehensive_benchmarks.rs:421`

**Why:** Phase 65 simplify review flagged the inconsistency — `LATEST_PROTOCOL_VERSION = "2025-11-25"` but `DEFAULT_PROTOCOL_VERSION = "2025-03-26"`. Mechanically bumping `DEFAULT` to match `LATEST` would be a silent behavior change for peers reaching the fallback path (they'd be assumed to speak the newer protocol before they've said so). The right fix is to delete the misleading abstraction, not flip its value.

**Requirements:** TBD

**Plans:** 11/11 plans complete

Plans:

- [ ] TBD (promote with `/gsd:review-backlog` when ready)

### Phase 999.2: TOON data format feasibility and SDK integration (BACKLOG)

**Goal:** Investigate whether PMCP should add built-in support for TOON as an alternative wire format to JSON for MCP tool outputs and resource payloads, specifically to optimize performance of MCP Apps that often ship large JSON payloads. Deliver a spike report + recommendation (adopt / pilot / reject), and if adoption is recommended, a follow-up implementation plan for a feature-gated `toon` format with encoder/decoder integrated into `Content`, tool output serialization, and the MCP App bridge so servers and Apps can opt in with a single flag.

**Motivation:**

- MCP Apps frequently serialize large structured payloads (tables, datasets, chart data) — the dataviz, hotel gallery, and venue map examples are all 10–100 KB of JSON per response
- Both the LLM context window and the UI widget render path benefit from smaller payloads: fewer tokens consumed by tool output, faster postMessage bridge transfer, faster widget mount
- TOON (Token-Oriented Object Notation) is designed explicitly for this use case — schema-aware compression that encodes repeated keys and types once, yielding ~30–60% size reductions on tabular data compared to JSON
- If adoption works, MCP servers would flip a flag per-tool or per-resource to switch output format, and MCP Apps would transparently decode on the bridge side

**Research questions (spike scope, not implementation):**

1. Maturity of TOON — is the spec stable enough to commit a feature-gated SDK integration? Is there a Rust encoder/decoder crate, or would PMCP need to author one?
2. Compatibility — can TOON payloads ride over existing MCP `Content` variants (probably via a new `TextContent` MIME type or a new `Content::Toon` variant), or does it need protocol changes that break v2025-11-25 compat?
3. Measurement — what are the realistic size/token savings on representative MCP App payloads (dataviz, gallery, map from existing examples)? Do LLMs tokenize TOON efficiently, or does the token savings on the wire get lost when Claude/GPT re-tokenize the decoded content?
4. Widget-side decoder — can the MCP App bridge (TypeScript) decode TOON in-browser without a heavy dependency, or is this a non-starter for the WASM/iframe sandbox?
5. Opt-in UX — what does `#[mcp_tool(output_format = "toon")]` or equivalent look like on the server side? What's the per-call server-side negotiation story?

**Why backlog (not an active phase):**

- The user's framing is explicitly exploratory ("let's investigate if we can add it as a built-in support")
- TOON is a newer format — the feasibility spike should happen before committing a phase slot
- The v2.1 rmcp Upgrades milestone is scoped to documentation polish; runtime data-format work doesn't belong there
- Natural home after spike: either seed of a "v2.2 Payload Optimization" milestone or early phase of a milestone focused on MCP Apps performance

**Promotion path:** Run `/gsd:discuss-phase 999.2` to gather context, then `/gsd:research-phase 999.2` for the spike, then promote via `/gsd:review-backlog` into an active milestone with a concrete Phase N number.

**Requirements:** TBD (depends on spike outcome)

**Plans:** 6 plans

Plans:

- [ ] TBD (promote with `/gsd:review-backlog` when ready)

### Phase 69: rmcp parity research — ergonomics gap analysis + follow-on phase proposals

**Goal:** Produce a rigorous, evidence-backed gap matrix comparing pmcp vs rmcp on *ergonomics* (macro DX, builder APIs, typed wrappers, handler shapes, state/extra patterns) and use it to propose 2–4 concrete follow-on phases to close the credibility/DX gap. Transports, examples polish, and docs coverage are intentionally out of scope — Phase 68 handles those surfaces at the polish layer.

**Deliverables:**

- `69-RESEARCH.md` — gap matrix (per-feature: rmcp approach, pmcp approach, gap severity, evidence citations)
- `69-PROPOSALS.md` — 2–4 follow-on phase proposals with goals, scope, and rough success criteria, ready to slot into v2.1 or seed v2.2

**Requirements**: TBD (derived from research findings; expected to seed new v2.1/v2.2 requirement IDs)
**Depends on:** Phase 68
**Plans:** 3 plans

Plans:

- [x] 69-01-PLAN.md — Produce the rmcp vs pmcp ergonomics gap matrix (69-RESEARCH.md) across 6 surfaces
- [x] 69-02-PLAN.md — Derive follow-on phase proposals from High-severity gaps (69-PROPOSALS.md)
- [x] 69-03-PLAN.md — Quality gate + land PARITY-* requirement IDs + update STATE/PROJECT

### Phase 70: Add Extensions typemap and peer back-channel to RequestHandlerExtra (PARITY-HANDLER-01)

**Goal:** Extend `RequestHandlerExtra` with two drop-in additive capabilities — a typed-key `Extensions` map (HANDLER-02) for request-scoped user data crossing middleware/handler boundaries, and an optional `PeerHandle` back-channel (HANDLER-05) exposing `sample` / `list_roots` / `progress_notify` from inside tool/prompt/resource handlers — without breaking any existing `::new(...)` or `::with_session(...)` call site. Restructured from 3 plans to 4 plans after cross-AI review (70-REVIEWS.md) + codebase verification (70-REVIEW-VERIFICATION.md) confirmed 5 of Codex's HIGH findings: the original plan set assumed an outbound `ServerRequest` transport + response-correlation layer that does not exist in the live codebase. Plan 02 (NEW) builds that foundational plumbing before Plan 03 wires the peer.
**Requirements**: PARITY-HANDLER-01
**Depends on:** Phase 69
**Plans:** 4 plans

Plans:

- [x] 70-01-PLAN.md — Extensions typemap on both RequestHandlerExtra structs + #[non_exhaustive] + accessor parity + 5 proptests + refactor 12 struct-literal test sites (Wave 1)
- [x] 70-02-PLAN.md — ServerRequestDispatcher (outbound ServerRequest + response correlation) + Server::run drain-to-transport + route TransportMessage::Response through dispatcher (NEW plan from reviews replan — addresses Codex Findings 2+3) (Wave 2)
- [x] 70-03-PLAN.md — PeerHandle trait + DispatchPeerHandle delegating to Plan 02 dispatcher + conditional .with_peer(...) at 9 dispatch sites + dispatch-path round-trip integration test (Wave 3)
- [x] 70-04-PLAN.md — Examples s42 + s43 (s43 uses real ToolHandler per Codex Finding 5) + fuzz target + rustdoc migration prose with explicit semver posture + make quality-gate (Wave 4)

### Phase 71: Rustdoc fallback for #[mcp_tool] tool descriptions (PARITY-MACRO-01)

**Goal:** Enable `#[mcp_tool]` to harvest the attached function's rustdoc as the tool description when the `description = "..."` attribute is omitted — eliminating forced duplication where a well-documented tool fn must repeat its description in both the rustdoc block and the macro attribute. Preserves precedence (explicit attribute wins over rustdoc), fails with a clear error when neither is present, and remains backwards-compatible with all existing call sites. Derived from 69-PROPOSALS.md Proposal 3 (MACRO-02, High severity).
**Requirements**: PARITY-MACRO-01
**Depends on:** Phase 70
**Plans:** 4/4 plans complete

Plans:

- [x] 71-01-PLAN.md — Create new sibling crate `crates/pmcp-macros-support/` (non-proc-macro) holding the pure `extract_doc_description` normalization helper with unit tests + proptest invariants — resolves HIGH-1 via Option A so proc-macro crate API restrictions don't block property/fuzz consumers (Wave 1)
- [x] 71-02-PLAN.md — `pmcp-macros` adds path dep on `pmcp-macros-support` + single shared `resolve_tool_args` resolver in `mcp_common.rs`; both parse sites (`mcp_tool.rs` standalone + `mcp_server.rs::parse_mcp_tool_attr` impl-block) delegate to it; integration tests lock symmetry (MEDIUM-1) (Wave 2)
- [x] 71-03-PLAN.md — 4 trybuild compile-fail snapshots (existing regenerated + new empty-args + new non-empty-args + regenerated multi-args) + README migration section with Limitations subsection + mixed-shape fuzz target `rustdoc_normalize.rs` (MEDIUM-2 + MEDIUM-3 + LOW-3) (Wave 3)
- [x] 71-04-PLAN.md — Workspace `pmcp`-dependency ripple audit + version bumps (pmcp 2.3.0→2.4.0 MINOR per MEDIUM-4, pmcp-macros 0.5.0→0.6.0, new pmcp-macros-support 0.1.0, concurrent downstream patch bumps cargo-pmcp 0.6.0→0.6.1 + mcp-tester 0.5.0→0.5.1 per CLAUDE.md §"Version Bump Rules") + CHANGELOG entry + REQUIREMENTS.md closure + `make quality-gate` (HIGH-2 + MEDIUM-4) (Wave 4)

### Phase 72: Investigate rmcp as foundations for pmcp - evaluate using rmcp for protocol level while focusing pmcp on pragmatic batteries-included SDK for enterprise use cases ✓ COMPLETE

**Status:** COMPLETE (2026-04-19) — **Recommendation: D** (Maintain pmcp as authoritative Rust MCP SDK; do not migrate onto rmcp). 7/9 decision thresholds resolved; T6/T7 remain UNKNOWN per 72-CONTEXT.md. Slice 1 spike executed — serde `params: null` round-trip fails against rmcp 1.5.0, downgrading inventory row 1 from EXACT to compatible-via-adapter. Phase 69's parity phases (70, 71, CLIENT-02) remain the forward path. See `.planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-RECOMMENDATION.md`.

**Goal:** Produce a research/decision-only recommendation on whether pmcp's protocol layer should be refactored to sit on top of rmcp 1.5.0 — repositioning pmcp + mcp-tester + mcp-preview + cargo-pmcp as a pragmatic, batteries-included, enterprise-focused SDK built *on top of* rmcp rather than alongside it. Deliverables are 7 markdown documents (CONTEXT, inventory, strategy matrix, PoC proposal, PoC results, decision rubric, final recommendation). If the recommendation is adopt (A/B/C1/C2), migration itself is scoped as a separate future v3.0 phase; if stay (D), Phase 69's parity phases remain the path forward.
**Requirements**: RMCP-EVAL-01, RMCP-EVAL-02, RMCP-EVAL-03, RMCP-EVAL-04, RMCP-EVAL-05
**Depends on:** Phase 71
**Plans:** 3/3 plans complete

Plans:

- [x] 72-01-PLAN.md — Seed RMCP-EVAL-01..05 in REQUIREMENTS.md; produce 72-INVENTORY.md (inversion inventory, >=15 pmcp module families with file:line + rmcp evidence) and 72-STRATEGY-MATRIX.md (5 options x 5 criteria = 25 cells, no TBD) (Wave 1)
- [x] 72-02-PLAN.md — Produce 72-POC-PROPOSAL.md (3 slices, each <=500 LOC, at least one <=3 days, with LOC/Files/Pass/Fail/Time-box fields) and 72-DECISION-RUBRIC.md (>=5 falsifiable thresholds, each followed by Data source) (Wave 2)
- [x] 72-03-PLAN.md — Produce 72-RECOMMENDATION.md (RMCP-EVAL-05) — opens with `**Recommendation:** <A|B|C|D|E>`, contains 5 per-criterion justification subsections citing T-IDs + inventory/matrix rows, lists UNRESOLVED thresholds, and names the next-phase handoff (Wave 3)

### Phase 72.1: Finalize landing support (INSERTED)

**Goal:** Ship CR-03 rev-2 — replace build-time `NEXT_PUBLIC_*` env vars in the landing Next.js template with a runtime `fetch('/landing-config')` via a new required shared hook `useLandingConfig`, fix 3 stale rustdoc references in `cargo-pmcp/src/landing/config.rs`, and bump `cargo-pmcp` 0.8.0 -> 0.8.1 (patch, additive). Unblocks pmcp.run Phase 71 UAT Test 7 and Cost Coach production launch.
**Requirements**: LAND-CR03-01
**Depends on:** Phase 72
**Plans:** 1/1 plans complete

Plans:

- [x] 72.1-01-PLAN.md — Create `lib/useLandingConfig.ts` hook; rewrite 4 consumers (signup, callback, connect [server->client flip], Header [conditional button]); fix 3 rustdoc comments in `src/landing/config.rs`; bump `Cargo.toml` 0.8.0 -> 0.8.1; run `make quality-gate` + `cargo doc` + template `tsc`/`next build` + grep guardrails G1..G6 + manual AC-11 offline gate (Wave 1)

### Phase 74: Add cargo pmcp auth subcommand with multi-server OAuth token management

**Goal:** Consolidate OAuth handling for cargo-pmcp's server-connecting commands into a dedicated `auth login/logout/status/token/refresh` command group with a per-server-keyed token cache. Add SDK-level Dynamic Client Registration (RFC 7591) so any PMCP-built client can auto-register, and expose it via a `--client <name>` flag on `auth login` for testing pmcp.run's client-branded login pages.
**Requirements**: SDK-DCR-01, CLI-AUTH-01
**Depends on:** Phase 72.1
**Plans:** 3/3 plans complete

Plans:

- [x] 74-01-PLAN.md — SDK DCR: OAuthConfig refactor (client_id Option), DcrRequest/DcrResponse re-export, auto-fire DCR in OAuthHelper, unit/property/fuzz/mockito-integration tests, examples/c08_oauth_dcr.rs, CHANGELOG entry (Wave 1, pmcp crate)
- [x] 74-02-PLAN.md — CLI auth group: new commands/auth_cmd/ module (login/logout/status/token/refresh + TokenCacheV1 cache with atomic writes & URL normalization), main.rs wiring, resolve_auth_middleware cache fallback with near-expiry auto-refresh, pentest.rs migration to shared AuthFlags, tempfile promoted to regular dep, mockito+cli integration tests (Wave 2, cargo-pmcp crate)
- [x] 74-03-PLAN.md — Release coordination: bump pmcp 2.4.0→2.5.0 and cargo-pmcp 0.8.1→0.9.0, update cargo-pmcp pmcp dep pin to 2.5.0, finalize CHANGELOG date, run make quality-gate to match CI exactly (Wave 3)

### Phase 73: Typed client helpers + list_all pagination (PARITY-CLIENT-01)

**Goal:** Ship additive, non-breaking `Client` ergonomics (pmcp 2.6.0): four typed-input helpers (`call_tool_typed`, `call_tool_typed_with_task`, `call_tool_typed_and_poll`, `get_prompt_typed`), four auto-paginating list helpers (`list_all_tools`, `list_all_prompts`, `list_all_resources`, `list_all_resource_templates`) with a bounded `max_iterations` safety cap, and a new `ClientOptions` config struct (`#[non_exhaustive]`) wired through a new `Client::with_client_options` constructor. Closes the client-side rmcp-parity DX gap (PARITY-CLIENT-01).
**Requirements**: PARITY-CLIENT-01
**Depends on:** Phase 74
**Plans:** 3/3 plans complete

Plans:

- [x] 73-01-PLAN.md — ClientOptions scaffold + new Client::with_client_options constructor + four typed helpers (call_tool_typed / _with_task / _and_poll / get_prompt_typed) with doctests, unit tests, and one property test (Wave 1, pmcp crate)
- [x] 73-02-PLAN.md — Four list_all_* auto-paginating helpers with max_iterations cap enforcement (T-73-01 DoS mitigation); integration test file tests/list_all_pagination.rs; two property tests (flat-concatenation + cap-enforcement); new fuzz target fuzz/fuzz_targets/list_all_cursor_loop.rs (Wave 2, pmcp crate)
- [x] 73-03-PLAN.md — Release coordination: examples/c09_client_list_all.rs (avoids c08 collision) + examples/c02_client_tools.rs update + README index; bump pmcp 2.5.0→2.6.0 across all 8 pin lines in 7 Cargo.toml files; CHANGELOG v2.6.0 entry; REQUIREMENTS.md §55 D-15 doc-fix (call_prompt_typed → get_prompt_typed); README Key Features bullet; make quality-gate (Wave 3)

### Phase 75: Fix PMAT issues

**Goal:** Restore the auto-generated `Quality Gate: passing` README badge by remediating PMAT findings (cognitive complexity is the gating dimension; SATD, duplicate, entropy, sections are best-effort within waves). After this phase, `pmat quality-gate --fail-on-violation --checks complexity` exits 0 and a CI gate prevents regression.
**Requirements**: None (quality-debt remediation; must_haves derived from CONTEXT.md decisions D-01..D-09)
**Depends on:** Phase 74
**Plans:** 6 plans

Plans:

- [x] 75-00-PLAN.md — Wave 0: Baseline + spike (PMAT path-filter empirical test, insta snapshot baseline for pmcp-macros, semantic regression baseline for pmcp-code-mode, PMAT version pin in CI) — completed 2026-04-23 — D-09 resolved (include_works=false, .pmatignore is the only honored filter), D-10 resolved D-10-B (PMAT ignores #[allow] — SCOPE EXPANSION DETECTED), D-11 resolved D-11-B (bare gate fails on 5 dimensions — Wave 5 must patch quality-badges.yml)
- [x] 75-01-PLAN.md — Wave 1: src/ + pmcp-macros/ refactors — completed 2026-04-24. 20 hotspots cleared to ≤25 via P1-P3 (zero P5 usage, zero escapees). Delta: PMAT complexity 94→75 (−19). Task 1a-C explicitly skipped per addendum Rule 1 (migrated to Phase 75.5 Category A); pre-existing bare #[allow] at streamable_http_server.rs:1004 removed. Macro expansion snapshots byte-identical (Wave 0 contract preserved).
- [x] 75-02-PLAN.md — Wave 2: cargo-pmcp/ refactors — completed 2026-04-24. 40 hotspots cleared to ≤25 via P1-P4 (zero P5 usage, zero escapees). Both monsters (check.rs::execute cog 105→≤25, handle_oauth_action cog 91→≤25) decomposed. Delta: PMAT complexity-gate 75→29 (−46); cargo-pmcp cog>25 40→0. `make quality-gate` exits 0. Shared scan_for_package helper established in cloudflare/init.rs (3-bird kill).
- [x] 75-03-PLAN.md — Wave 3: pmcp-code-mode/ refactors — completed 2026-04-24. All 5 named hotspots cleared to ≤25 via P6 (eval.rs) + P1 (policy_annotations.rs, schema_exposure.rs); zero P5 usage; zero escapees. Both eval-monsters decomposed: evaluate_with_scope 123→17, evaluate_array_method_with_scope 117→≤25. Delta: PMAT complexity-gate 29→22 (−7). Pre-existing pmcp-code-mode lint debt (18 lib + 28 test errors + 3 dead-code) cleared in opening sweep. `make quality-gate` exits 0. Wave 0 semantic-regression baseline byte-identical (34 passed throughout).
- [x] 75-04-PLAN.md — Wave 4: scattered crate hotspots + examples/fuzz handling per Wave 0 spike + SATD triage per D-04 + final pre-Wave-5 gate verification — completed 2026-04-25. 5 plan-named hotspots cleared to ≤25 (P1+P4) plus 8 additional warning-level cog 24-25 violations refactored under Rule 3 (gate-counted but out-of-plan-list). `.pmatignore` configured for fuzz/+packages/+examples/ per Wave 0 chosen_path: (a). 11 in-scope SATDs migrated to `// See #NNN` refs against 3 umbrella issues (paiml/rust-mcp-sdk#247/#248/#249); 14 SATD matches classified as out-of-D-04-scope scaffold/template content. Delta: PMAT complexity-gate 22→0 (−22); aggregate Phase 75 delta 94→0. `pmat quality-gate --fail-on-violation --checks complexity` exits 0. Wave 5 can flip the README badge.
- [~] 75-05-PLAN.md — Wave 5: D-07 enforcement (CI gate in ci.yml, regression-PR fail-closed test, badge-flip confirmation, CLAUDE.md docs update). NOTE (D-11-B): must also patch `.github/workflows/quality-badges.yml:~72` bare gate → `--checks complexity` per 75-ADDENDUM-D10B.md Rule 5. — substantially completed 2026-04-25. Tasks 5-01 (ci.yml gate + D-11-B quality-badges.yml alignment), 5-04 (CLAUDE.md docs) landed; Task 5-02 replanned mid-execution (option A → option B per user) — fork-internal CI did not fire due to fork-main divergence; switched to local-pmat empirical evidence (cog-77 fixture exits 1 by name) recorded in 75-05-GATE-VERIFICATION.md. Task 5-03 (badge flip on README) **deferred** until Wave 5 lands on `paiml/rust-mcp-sdk:main` — operator follow-up: trigger `gh workflow run quality-badges.yml -R paiml/rust-mcp-sdk` post-merge and append observation to GATE-VERIFICATION.md.

### Phase 75.5: PMAT ex-P5 refactor backlog

**Goal:** Absorb the refactor work that Phase 75 could not land under P5 (`#[allow(clippy::cognitive_complexity)]` + `// Why:`) because Wave 0 D-10 spike proved PMAT 3.15.0 ignores the allow attribute. Category A: the 12 pre-existing bare-allow sites in `src/` (orchestrator-verified count; PATTERNS.md said "13" but the 13th — `streamable_http_server.rs:1004 handle_post_with_middleware` — was already refactored in Phase 75 Wave 1a). Category B: escapees logged to `75.5-ESCAPEES.md` during Plans 75-01..75-04 — empty (zero entries logged across Phase 75 Waves 1-4). Each Category A site either refactors to ≤25 or has the ineffective `#[allow]` removed because the underlying function already simplified.
**Requirements**: None (quality-debt remediation, sibling of Phase 75)
**Depends on:** Phase 75 Waves 1-4 complete (so Category B is known — confirmed empty as of 2026-04-25). MAY land in parallel with Phase 75 Wave 5 or before it.
**Plans:** 1/1 plan complete

Plans:

- [x] 75.5-01-PLAN.md — Wave 1: 12 Category-A bare `#[allow(clippy::cognitive_complexity)]` attributes removed from src/ (server/, server/transport/, shared/, client/) — completed 2026-04-25. All 12 sites resolved by single-line attribute deletion (no refactor triggered — clippy pedantic+nursery quiet on `--features full` post-removal, confirming all underlying functions sit at cog ≤25). `make quality-gate` exit 0; `pmat quality-gate --fail-on-violation --checks complexity` exit 0 (PMAT 3.15.0); `grep -rn '#[allow(clippy::cognitive_complexity)]' src/` 0 matches. ESCAPEES.md (Category B) unchanged at 0 entries. Two pre-existing environmental test failures (mcp-e2e-tests::chess chromiumoxide browser archive missing; pmcp-tasks::store::redis/dynamodb Connection refused) classified out-of-scope per deviation-rules SCOPE BOUNDARY — neither touches src/server/, src/shared/, or src/client/. Commits: fae333fa (Task 1: server/+server/transport/), 7a0cc362 (Task 2: shared/+client/).

### Phase 76: cargo-pmcp IAM declarations — servers declare IAM needs in deploy.toml

**Goal:** Ship pmcp-run CR `CLI_IAM_CHANGE_REQUEST.md` in one phase — Part 1 adds a stable `McpRoleArn` CfnOutput (`Export.Name = pmcp-${serverName}-McpRoleArn`) to both generated CDK stack templates (pmcp-run + aws-lambda), unblocking bolt-on stacks via `Fn::ImportValue`. Part 2 adds an optional `[iam]` section to `.pmcp/deploy.toml` with three repeated tables (`[[iam.tables]]`, `[[iam.buckets]]`, `[[iam.statements]]`) that translate to `addToRolePolicy` calls on the Lambda execution role, plus a new `cargo pmcp validate deploy` subcommand that hard-errors on IAM footguns (Allow-*-*, bad effects, malformed actions). Backward compatible (empty default) per D-05 byte-identity. Target: cargo-pmcp 0.10.0 (additive minor bump).
**Requirements**: PART-1 (McpRoleArn export), PART-2 (declarative `[iam]` section + validator)
**Depends on:** Phase 75
**Plans:** 5/5 plans complete

Plans:

- [x] 76-01-PLAN.md — Wave 1: Part 1 — McpRoleArn CfnOutput in both template branches + `render_stack_ts` renderer extraction + D-03 aws-iam import fix + Wave 1 golden-file baseline (D-05 anchor)
- [x] 76-02-PLAN.md — Wave 2: Full IamConfig schema (TablePermission / BucketPermission / IamStatement) wired into DeployConfig with `skip_serializing_if` to preserve D-05 + serde roundtrip integration tests
- [x] 76-03-PLAN.md — Wave 3: Translation rules (`deployment/iam.rs::render_iam_block`) emitting D-02 4-action DynamoDB lists + S3 object-level ARNs + passthrough statements, wired into `render_stack_ts` via a single `{iam_block}` named placeholder + per-rule unit tests + 9 proptests
- [x] 76-04-PLAN.md — Wave 4: Validator (`validate` + `Warning`) enforcing 6 CR-locked hard-error rules + 2 warning classes + `ValidateCommand::Deploy` subcommand + DeployExecutor hook blocking deploy on hard errors + 29 new tests covering T-76-02 mitigation
- [x] 76-05-PLAN.md — Wave 5: `fuzz_iam_config` libfuzzer target + corpus seeds + `deploy_with_iam` runnable example + cost-coach fixture + DEPLOYMENT.md IAM Declarations section + README.md pointer + CHANGELOG 0.10.0 entry + version bump + final `make quality-gate`

### Phase 77: Add cargo pmcp configure commands

Developers using cargo pmcp across multiple deployment and upload targets (dev/prod, per-server) currently struggle to maintain and switch between environments. Design and implement `cargo pmcp configure` (modeled after `aws configure`) that lets a developer:

(1) define named targets (e.g., dev, prod, staging) with target-specific configuration: pmcp.run discovery endpoint URL (PMCP_API_URL like https://ipwojemcm6.execute-api.us-west-2.amazonaws.com or its /.well-known/pmcp-config variant), AWS CLI profile, region, and any target-specific credentials/secrets;

(2) switch quickly between targets with a per-workspace selection (one server can stay in dev mode pointing at a dev pmcp.run while a sibling server in the same monorepo deploys to prod);

(3) extend cleanly to non-pmcp.run target types: aws-lambda direct deploy with different AWS profiles, Google Cloud Run, or future targets;

(4) integrate with existing cargo pmcp deploy / cargo pmcp pmcp.run upload flows so they read the active target instead of hardcoded URLs/profiles.

Scope likely includes: a config schema (TOML in workspace .pmcp/ or user ~/.config/pmcp/), `cargo pmcp configure add|use|list|remove|show`, env var override support (PMCP_TARGET=name), and explicit precedence rules between workspace, user, and env.

**Goal:** Ship a `cargo pmcp configure` command group (add/use/list/show) that manages named deployment targets in `~/.pmcp/config.toml` and a per-workspace `.pmcp/active-target` marker; integrates with `cargo pmcp deploy` and `pmcp.run upload` via a precedence-merge resolver (ENV > flag > target > deploy.toml) and a fixed-order header banner; maintains zero-touch backward compatibility for users without a config.toml.
**Requirements**: REQ-77-01, REQ-77-02, REQ-77-03, REQ-77-04, REQ-77-05, REQ-77-06, REQ-77-07, REQ-77-08, REQ-77-09, REQ-77-10
**Depends on:** Phase 76
**Plans:** 9/9 plans complete

Plans:

- [x] 77-01-PLAN.md — Mint REQ-77-01..REQ-77-10 in REQUIREMENTS.md; bump cargo-pmcp 0.10.0 → 0.11.0; CHANGELOG stub
- [x] 77-02-PLAN.md — Rename existing deploy `--target` to `--target-type` (with alias); add new global `--target` named-target flag on Cli
- [x] 77-03-PLAN.md — Module skeleton + TargetConfigV1 schema (TOML, atomic write, 0o600) + workspace utility
- [x] 77-04-PLAN.md — `configure add` (interactive + flag-driven, raw-credential validator) + `configure use` (workspace marker)
- [x] 77-05-PLAN.md — `configure list` (text + stable JSON) + `configure show` (raw + merged-with-attribution placeholder)
- [x] 77-06-PLAN.md — Resolver (precedence walk, env injection helper) + banner (D-13 fixed-order, OnceLock idempotent) + show.rs enrichment
- [x] 77-07-PLAN.md — Top-level Cli wiring: register Configure variant, dispatch arm, env injection in main.rs, banner emission in deploy/mod.rs
- [x] 77-08-PLAN.md — Integration tests (full lifecycle, zero-touch, concurrent writes) + fuzz target + working multi-target-monorepo example
- [x] 77-09-PLAN.md — DRY cleanup (shared validate_target_name) + rustdoc audit + CHANGELOG date + `make quality-gate` certification + manual interactive UX checkpoint

### Phase 78: cargo pmcp test apps --mode claude-desktop: detect missing MCP Apps SDK wiring in widgets

Goal: Catch the silent-fail bug where a widget passes `cargo pmcp test apps` and renders fine in ChatGPT but breaks in Claude Desktop / claude.ai because the widget HTML never imports `@modelcontextprotocol/ext-apps`, never instantiates `App`, and never registers the four required handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`) before `connect()`.

Scope (this phase):

1. Promote `AppValidationMode::ClaudeDesktop` from placeholder ("same as Standard for now" at `crates/mcp-tester/src/app_validator.rs:28-29`) to a real strict mode.
2. In `cargo-pmcp/src/commands/test/apps.rs`, fetch each App-capable tool widget body via `resources/read` and pass `Vec<(uri, html)>` into the validator (keeps validator a pure function; ~30 LOC of plumbing).
3. Add static script-block checks behind `--mode claude-desktop`:
   - Imports `@modelcontextprotocol/ext-apps` OR has >=3 of the 4 protocol-handler property assignments (handles minified bundles where the import string is preserved but identifiers are renamed; both signals survive Vite singlefile minification).
   - Constructs `new App({...})` with non-empty Implementation.
   - Registers `onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror` (ERROR each).
   - Registers `ontoolresult` (WARN - some widgets render from `getHostContext().toolOutput`).
   - Calls `app.connect()` (ERROR).
   - "ChatGPT-only channels and no ext-apps wiring" -> ERROR in `claude-desktop` mode, OK in `chatgpt` mode.
4. Severity calibration matches existing pattern: `Standard` mode = WARN (MCP Apps is optional in the spec); `ClaudeDesktop` mode = ERROR - mirrors how `Standard` vs `ChatGpt` treat `openai/*` keys today.
5. Polish: error messages link to specific anchors in `src/server/mcp_apps/GUIDE.md` (especially the "Critical: register all four handlers before connect()" warning at line 185); update README and `cargo pmcp test apps --help` to document the new mode and recommend it as the pre-deploy check for servers shipping to Claude clients.

Out of scope (defer to a later phase):

- `PreviewMode::ClaudeDesktop` host emulator (postMessage init/tool-result/teardown simulation in `crates/mcp-preview/src/server.rs`). User wants to think about it later and may unify the preview UX across ChatGPT/Claude modes rather than add a third mode.

Reference / context:

- Proposal from the Cost Coach team: `/Users/guy/projects/mcp/cost-coach/drafts/proposal-pmcp-mcp-app-widget-validation.md`
- Failing widget bundle + working fix available from Cost Coach as a regression fixture (request via the proposal author).
- Verified state of the codebase: `AppValidationMode::ClaudeDesktop` is wired into Display/FromStr/CLI parsing but has zero behavior behind it; `AppValidator::validate_tools` only consumes `&[ResourceInfo]` metadata - no `resources/read` call, so widget HTML is never inspected.

ALWAYS requirements (per CLAUDE.md):

- Unit tests for each new check (positive and negative cases for each handler / SDK signal).
- Property tests for the script-block scanner (must not panic on arbitrary HTML/JS input; idempotent on normalized whitespace).
- Fuzz target for the regex/AST scan path.
- A working example: a `cargo run --example` (or fixture under `examples/`) showing a deliberately-broken widget that fails `--mode claude-desktop` and a corrected one that passes - same widget pair the Cost Coach team will provide.

Acceptance criteria:

- The Cost Coach reproducer (broken widget) FAILS `cargo pmcp test apps --mode claude-desktop` with errors that name the missing handler(s).
- The corrected version PASSES.
- `cargo pmcp test apps` (no flag, Standard mode) still passes for both - no regression for the permissive default.
- `--mode chatgpt` behavior unchanged.
- README + `--help` document the new mode.

**Goal:** Promote `AppValidationMode::ClaudeDesktop` from a placeholder to a real strict mode that statically inspects each App-capable widget HTML body (fetched via `resources/read`) for the `@modelcontextprotocol/ext-apps` import, the `new App({...})` constructor, the four required protocol handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`), and the `app.connect()` call — emitting ERROR (vs WARN in Standard mode) on missing signals so widgets shipping to Claude Desktop / Claude.ai are caught before deploy.
**Requirements**: PHASE-78-AC-1, PHASE-78-AC-2, PHASE-78-AC-3, PHASE-78-AC-4, PHASE-78-AC-5, PHASE-78-ALWAYS-UNIT, PHASE-78-ALWAYS-PROPERTY, PHASE-78-ALWAYS-FUZZ, PHASE-78-ALWAYS-EXAMPLE
**Depends on:** Phase 77
**Plans:** 7/11 plans executed (cycle-1 03/04 done; cycle-1 wave 4 plan 08 paused at checkpoint; cycle-2 plans 09-11 added 2026-05-02)

Plans:
**Wave 1**

- [x] 78-01-PLAN.md — Validator core: extend `AppValidator` with `validate_widgets`, regex-based scanner, mode-driven severity (Wave 1)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 78-02-PLAN.md — CLI plumbing: wire `read_widget_bodies` into `cargo pmcp test apps` (Wave 2)
- [x] 78-04-PLAN.md — Docs polish: README sections, `--help` long-text, GUIDE.md anchor expander (Wave 3, parallel with 78-03)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 78-03-PLAN.md — ALWAYS requirements: fixtures, property tests, fuzz target, working example (Wave 3)

### Phase 79: cargo pmcp deploy: widget pre-build + post-deploy verification (build half: auto-detect widget/ and widgets/ only, package-manager runner, generated build.rs with cargo:rerun-if-changed via env-var path resolution, [[widgets]] config with explicit embedded_in_crates, doctor checks; verify half: warmup grace + test check + conformance + apps --mode claude-desktop, on_failure=fail default; depends on Phase 78; out of scope: auto-rollback, multi-target)

**Goal:** Close two silent-failure gaps in `cargo pmcp deploy` proven by Cost Coach: (A) deploy ships stale `widget/dist/*.html` because nobody ran `npm run build`; (B) Cargo's incremental cache holds a stale `include_str!`-built binary; (C) widget JS SDK is misconfigured but deploy reports success because nothing probes the live endpoint. Build half auto-detects widget directories, runs the lockfile-determined package manager, sets `PMCP_WIDGET_DIR` for cache invalidation via a generated `build.rs`. Verify half runs warmup → check → conformance → apps lifecycle after Lambda hot-swap and surfaces a screaming-loud LIVE-but-broken banner on failure (`on_failure="fail"` default) with the manual rollback command pre-printed.
**Requirements**: REQ-79-01..18 (locally-derived per CONTEXT.md "Implementation Decisions"; phase has no numbered REQUIREMENTS.md entries)
**Depends on:** Phase 78
**Plans:** 7/7 plans complete

Plans:

- [x] 79-00-PLAN.md — Master plan: wave structure, requirement-to-plan mapping, version bumps, locked planner decisions
- [x] 79-01-PLAN.md — Wave 1: test fixtures + config schema (`WidgetsConfig`, `PostDeployTestsConfig`, `OnFailure`, `TestOutcome`)
- [x] 79-02-PLAN.md — Wave 2: widget pre-build orchestrator + `--no-widget-build` / `--widgets-only` CLI flags + `PMCP_WIDGET_DIR` env-var contract
- [x] 79-03-PLAN.md — Wave 3: post-deploy verifier (subprocess-spawn `cargo pmcp test {check,conformance,apps}` via `current_exe()`) + 4 verify-half flags + WARN-at-deploy-START for `OnFailure::Rollback`
- [x] 79-04-PLAN.md — Wave 4: doctor `check_widget_rerun_if_changed` + `cargo pmcp app new` build.rs scaffold + runnable example + fuzz target + `cargo-pmcp 0.12.0` version bump + CHANGELOG

---

#### Phase 78 — Gap closure (Plans 05–08, added 2026-05-02)

After cost-coach team UAT against prod (`https://cost-coach.us-west.pmcp.run/mcp`, 8 widgets, 97 tests, 33 failures — all confirmed false positives), 5 gaps were filed in 78-VERIFICATION.md and 4 gap-closure plans were spawned. AC-78-1, AC-78-2, AC-78-3 fail at the binary boundary against real prod; library-boundary verification (9/9 truths) was already passing. The cost-coach prod evidence: bundled widgets contain mangled constructor identifiers (e.g. `new yl({name:"cost-coach-cost-summary",version:"1.0.0"})`) that defeat the v1 `new App\(` regex, the `[ext-apps]` package name only survives as a log-prefix string (not the import literal `@modelcontextprotocol/ext-apps`), and the v1 SDK-detection failure cascades to all 8 handler/connect checks, producing `1 false negative → 8× false negatives` per affected widget.

**Plans (all `gap_closure: true`):**

**Wave 1**

- [x] 78-05-PLAN.md — RED-phase regression fixtures: 3 bundled HTML fixtures + `app_validator_widgets_bundled.rs` integration tests asserting verdict shape per fixture × mode; tests MUST FAIL today (G5)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 78-06-PLAN.md — Validator core fixes (G1+G2+G3): minification-resistant SDK-presence signals (`[ext-apps]` log prefix + `ui/initialize` + `ui/notifications/tool-result` method literals); mangled-id-tolerant constructor regex; eliminate SDK-to-handler/connect cascade

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 78-07-PLAN.md — `cargo pmcp test apps --widgets-dir <path>` source-scan flag (G4): scan `<path>/*.html` instead of fetching via `resources/read`; mirrors `cargo pmcp preview --widgets-dir` semantics; 3 CLI-boundary integration tests via `assert_cmd`

**Wave 4** *(blocked on Wave 3 completion)*

- [ ] 78-08-PLAN.md — ALWAYS coverage extension + docs + HUMAN-UAT re-bind: new `prop_g3_handler_detection_independent_of_sdk` proptest, `validate_widget_pair` example demos cost-coach prod-bundle shape, READMEs document `--widgets-dir`, `78-HUMAN-UAT.md` rewritten with 6 re-bound items including cost-coach prod re-verify (Test 6)

**Re-verification gate:** After Plan 06 lands, the cost-coach v1 run (97 tests, 33 false positives) must be re-executed and report zero false-positive failures on the 8 prod widgets. The 5 deferred AC-78-1..5 items are re-bound to the post-Plan-07 `--widgets-dir` path so binary-boundary verification no longer requires the deferred fixture binary `mcp_widget_server.rs.todo`.

#### Phase 78 — Gap closure cycle 2 (Plans 09-11, added 2026-05-02)

After cycle-1 closure (Plans 05-08 completed 2026-05-02), the operator re-ran Test 6 against `https://cost-coach.us-west.pmcp.run/mcp` and got the SAME 33 Failed rows. The cycle-1 synthetic fixtures didn't generalize to real Vite-singlefile prod output. Per-widget breakdown in `uat-evidence/2026-05-02-cost-coach-prod-rerun.md`: G2 constructor regex misses 8/8 prod widgets; G1 SDK signals miss 4/8. Diagnosis: Plan 05's fixtures were modeled from feedback-described shape, not bytes captured from prod — RED→GREEN passed against the model, missed reality. Cycle 2 binds the regression set to bytes captured from real prod.

**Plans (all `gap_closure: true`):**

**Wave 1**

- [ ] 78-09-PLAN.md — Real-prod fixture capture (RED phase): 6 cost-coach prod widget bundles fetched from live cost-coach prod (or local checkout) into `tests/fixtures/widgets/bundled/real-prod/` + CAPTURE.md provenance + 7 RED-phase integration tests (6 real-prod fixtures × claude-desktop + 1 cycle-1 no-regression sentinel) bound to those bytes; tests MUST FAIL today (G6)

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 78-10-PLAN.md — Validator G1+G2 generalization (GREEN phase): derive new SDK-presence + constructor patterns from real-prod CAPTURE.md grep evidence; widen mangled-id cap, add quoted-key tolerance + reordered-key support to G2; OR new G1 signals into has_sdk; preserve cycle-1 unit/property/integration tests; PMAT cog ≤ 25 + zero SATD; new G2-false-positive-guard property test guards against the widening risk

**Wave 3** *(blocked on Wave 2 completion)*

- [ ] 78-11-PLAN.md — ALWAYS-coverage extension + HUMAN-UAT cycle-2 rewrite + Test 6 re-verification checkpoint: extend `validate_widget_pair.rs` example with 6 cycle-2 real-prod widget runs + tally + success-path summary; rewrite `78-HUMAN-UAT.md` with cycle-2-explicit Test 6 acceptance bar (zero Failed rows on 8 cost-coach prod widgets); operator re-runs Test 6 against prod and resumes with `approved` (flips `gap_closure_validated: false → true`, routes to `/gsd-verify-work`) or `failed: <reason>` (routes to `/gsd-plan-phase 78 --gaps` for cycle 3)

**Re-verification gate (cycle 2):** Plan 11 Task 3 is the load-bearing gate. Operator runs `cargo pmcp test apps --mode claude-desktop https://cost-coach.us-west.pmcp.run/mcp` against real prod and confirms zero Failed rows on the 8 production widgets. On pass: phase 78 closes via `/gsd-verify-work`. On fail: phase 78 routes to a third gap-closure cycle with diagnosis in a new `uat-evidence/<date>-cost-coach-prod-cycle3-rerun.md` evidence file.

### Phase 81: Update pmcp-book and pmcp-course with v2 advanced topics (code-mode, tasks, skills)

**Goal:** [To be planned]
**Requirements**: TBD
**Depends on:** Phase 80
**Plans:** 10/10 plans complete

Plans:

- [x] TBD (run /gsd-plan-phase 81 to break down) (completed 2026-05-15)

**Cross-cutting constraints:**

- Every behavioral-prose claim about Tasks (SSE, serverless, owner binding, experimental.tasks, TaskSupport::*, tasks/result, tasks/cancel, tasks/get, poll interval, pollInterval, CreateTaskResult) still accurately describes current `pmcp-tasks` behavior (revision R-5 — prose drift, not just type-name drift).

### Phase 105: Task poll-decision classifier and durable-consumer docs

**Goal**: Make `TaskStatus::InputRequired` an actionable state for every consumer shape by factoring the terminal/pollable/input-required poll decision OUT of `Client::wait_for_task`'s loop into a shared, loop-free classifier — `Terminal { status } | InProgress { poll_hint } | InputRequired` (3 variants, unit `InputRequired`, no `Unpollable` — revised per CONTEXT.md D-03/D-05 after Codex review) — consumed internally by `wait_for_task` (D-05 single-decision discipline: the two poller shapes cannot drift) and callable per-poll by durable/replay consumers (Temporal-style `ctx.step`/`ctx.wait` loops that cannot block). Plus the docs half: a "Consuming tasks from a durable/replay workflow" page (rustdoc next to `wait_for_task` + pmcp-book) covering the typed-accessors-without-the-loop pattern, the replay-determinism caveat, and an explicit "when NOT to use `wait_for_task`" section. Note the classifier is a pure function of the polled `Task` — the terminal `CallToolResult` still comes from a separate `tasks/result` call the consumer owns (e.g., as its own memoized durable step).

**Scope fences (LOCKED):** no wire changes (`tasks/provide_input` explicitly REJECTED as spec-invention — polling-client input provision is an upstream spec gap; the classifier's `InputRequired` variant is the seam for when the WG standardizes it); no new `TaskStatus` variants; no change to `wait_for_task` blocking behavior or its `input_required` typed-error default (2.12.0 CR-01 fix stays).

**Origin:** pmcp.run dev-team request `~/Development/mcp/sdk/pmcp-run/.planning/notes/sdk-issue-durable-task-consumer-and-input-required.md` — Ask A accepted (this phase), Ask B (task elicitation round-trip) deferred as spec-shaped, Ask C answered; SDK response at `pmcp-run/.planning/notes/sdk-response-durable-task-consumer-and-input-required.md`.

**Requirements**: D-01..D-16 (CONTEXT.md decision set; no separate REQ IDs mapped)
**Depends on:** Phase 104 (shipped in pmcp 2.12.0)
**Plans:** 3/3 plans complete

Plans:

**Wave 1**

- [x] 105-01-PLAN.md — Pure poll-decision primitive: `TaskPollDecision` enum + `Task::poll_decision()` + `resolve_poll_interval()` in `src/types/tasks.rs` (Wave 1)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 105-02-PLAN.md — Rewrite `wait_for_task` as `match task.poll_decision()` + drift-pin regression test (Wave 2)
- [x] 105-03-PLAN.md — Runnable s48 example + "Durable and replay consumers" book section (Wave 2)

---

### Phase 80: SEP-2640 Skills Support

- [x] **Phase 80: SEP-2640 Skills Support** — Add the experimental Skills extension (SEP-2640) as a batteries-included PMCP feature. Includes (a) a one-line additive change to `ServerCapabilities` adding an `extensions` field parallel to `experimental`, (b) a new `Skill` / `SkillReference` / `Skills` DX layer behind a `skills` feature flag built as sugar over the existing `ResourceHandler` trait, (c) builder methods `.skill(...)` / `.skills(...)` / `.bootstrap_skill_and_prompt(...)` on `ServerCoreBuilder` with internal composition over any pre-existing `.resources(...)` handler, (d) the dual-surface pattern — same skill data exposed via both a SEP-2640 skill surface AND a parallel MCP prompt surface (for hosts that don't yet support SEP-2640) — with byte-equal-by-construction invariant, (e) paired `examples/s38_server_skills.rs` + `examples/c38_client_skills.rs` demonstrating three tiers of skills (hello-world, refunds, code-mode), and (f) integration test asserting both surfaces produce byte-equal content. (completed 2026-05-13)

**Goal:** A PMCP server author can register an Agent Skill in ~5 lines of code, and the same skill content is automatically reachable via two parallel surfaces: SEP-2640 skill resources (for capable hosts) and an MCP prompt (for everyone else). The two surfaces are derived from a single `Skill` value so they cannot drift.

**Depends on:** None (no protocol breaking change; additive `extensions` field is backward-compatible).

**Source of truth:** Spike findings packaged at `.claude/skills/spike-findings-rust-mcp-sdk/` (spikes 001 + 002 both VALIDATED). Reference implementation lives at `.planning/spikes/002-skill-ergonomics-pragmatic/src/main.rs` — the `Skill`, `SkillReference`, `Skills`, `SkillsHandler`, and `ComposedResources` types lift near-verbatim into the real implementation.

**Out of scope (deferred to v2):**

- SEP-2640 §4 archive distribution (`application/gzip` + base64 blob). Blocked by GAP #2 (`Content::Resource` has no `blob` field). The SEP marks archive mode as optional.
- `#[pmcp::skill]` procedural macro for compile-time SKILL.md validation. Worth a separate spike if compile-time validation is wanted.

## v2.2 Configuration-Only MCP Servers (In Progress)

**Milestone Goal:** Shift PMCP from a code-based SDK to one that lets enterprise developers build production-grade MCP servers for SQL databases from configuration + schema files alone — without writing Rust — while preserving PMCP's security, tools/resources/prompts/tasks/skills standards and offering pmcp.run hosting as a deployment target.

**Source of truth:** Validated spikes 003–006 (`.planning/spikes/00{3,4,5,6}-*/`) + auto-loaded `spike-findings-rust-mcp-sdk` skill. Reference implementation: the three production SQL-API servers under `pmcp-run/built-in/sql-api/servers/` (`open-images`, `imdb`, `msr-vtt`) — their `config.toml` shape is the load-bearing input contract for the toolkit lift.

**Critical invariants encoded across phases:**

- Toolkit `config.toml` schema is a **superset** of `pmcp-run/built-in/sql-api/servers/open-images/config.toml` — additive new keys allowed, **no renames** (REF-01).
- Pure-Rust Lambda is the deployment target — **no Docker, no testcontainers** (per `feedback_avoid_docker_pure_rust_lambda` memory).
- Dual-mode curated `[[tools]]` + `[code_mode]` long-tail split is **intentional**, not auto-conversion.
- SEP-2640 dual-surface invariant: prompt body **byte-equals** SKILL.md (SKLL-05).
- SEP-2640 §9: supporting files served via `resources/read` but **NOT** in `resources/list` (SKLL-06).

### v2.2 Phase Summary

- [x] **Phase 82: Builder DX Prerequisites** — Lift `tool_arc` / `prompt_arc` to public `ServerBuilder` + document in-process driver pattern so external toolkit authors stop writing 20-line delegating shims (completed 2026-05-18)
- [x] **Phase 83: Toolkit Core Lift (`pmcp-server-toolkit`)** — Promote `mcp-server-common` shape (~2.2k LoC) to a public crates.io-published SDK crate: AuthProvider, SecretsProvider, StaticResourceHandler, StaticPromptHandler, HMAC tokens, ToolInfo synthesis from `[[tools]]` config, code-mode policy wiring (completed 2026-05-18)
- [x] **Phase 84: SQL Connectors (Postgres / MySQL / Athena / SQLite)** — `SqlConnector` trait + `Dialect` enum + 3 per-backend crates (pure-Rust drivers, Lambda-friendly) + SQLite feature flag, with placeholder translation and dialect-aware code-mode prompt assembly (completed 2026-05-26)
- [x] **Phase 85: Shape A Pure-Config Binary + Reference Parity** — `pmcp-sql-server --config X --schema Y` zero-Rust binary; reproduce open-images end-to-end against the canonical reference scenarios (completed 2026-05-27)
- [x] **Phase 86: Shapes B/C/D — Scaffold, Library Example, Deploy** — `cargo pmcp new --kind sql-server` scaffolding (Shape B), ≤15-line `main.rs` example (Shape C), `cargo pmcp deploy` config-only target wiring with Phase 77 configure system (Shape D) (completed 2026-05-27)
- [ ] **Phase 87: Type 2 Authoring Skills MCP Server (`pmcp-config-helper`)** — SEP-2640 Skills MCP server for `config.toml` authoring: root SKILL.md + per-backend references + worked examples, byte-equal dual-surface invariant, §9 list-exclusion compliance, Type 1 `ai-agents/` updates
- [ ] **Phase 88: Dogfood — `crates/pmcp-server` on Toolkit** — Rewrite the SDK's own dev-tools MCP server on top of `pmcp-server-toolkit` with at least one config-driven tool surface; behavioral parity verified
- [ ] **Phase 89: Documentation, Migration Guide & Examples Index** — New book chapter + course tutorial covering the four shapes + per-backend recipes + deployment; README + crate-README config-first positioning; reference-implementation migration recipe (REF-03); examples/README + cargo-pmcp README polish

## Phase Details — v2.2 Milestone

### Phase 82: Builder DX Prerequisites

**Goal**: External toolkit authors can share an `Arc<dyn ToolHandler>` between `pmcp::ServerBuilder` and an in-process handler map without writing a 20-line delegating wrapper shim, and can drive a built `pmcp::Server` in integration tests via a documented public pattern.
**Depends on**: Phase 81 (v2.1 close); independent of any other v2.2 phase (this unblocks every later phase that uses `tool_arc` / `prompt_arc` in `pmcp-server-toolkit`)
**Requirements**: BLDR-01, BLDR-02, BLDR-03, BLDR-04
**Success Criteria** (what must be TRUE):

  1. A toolkit author can call `pmcp::ServerBuilder::tool_arc(name, Arc::new(handler))` on the public builder and share that same `Arc` with an in-process handler map — no delegating wrapper required
  2. A toolkit author can call `pmcp::ServerBuilder::prompt_arc(name, Arc::new(handler))` on the public builder with the same `Arc`-sharing semantics
  3. A toolkit integration test can drive a built `pmcp::Server` end-to-end through `tools/list` / `tools/call` flow via a public in-process driver OR via an officially documented handler-level testing pattern — no poking at private `Server::handle_request`
  4. The new builder methods are additive (no existing builder method signatures change) and ship as part of a minor `pmcp` version bump — the actual `Cargo.toml` version change and `CHANGELOG.md` entry are produced by the v2.2.x release branch per `CLAUDE.md` §"Release & Publish Workflow", NOT by Phase 82's implementation plans. (Phase 82 closes when its three plans land; the release that ships them is tagged separately.)
  5. All six `_arc` handler-registration paths (`tool_arc`, `prompt_arc`, `resources_arc`, `sampling_arc`, `auth_provider_arc`, `tool_authorizer_arc`) reach parity with `ServerCoreBuilder`
  6. `pmcp::Server::get_tool(name) -> Option<&Arc<dyn ToolHandler>>` exists, symmetric with the existing `get_prompt(name)`

**Plans**: 3 plans (1 complete)
Plans:

- [x] 82-01-PLAN.md — Lift six `_arc` methods + `Server::get_tool` + behavioral test + D-03 doctests (commits 8de9ad79..f0dc4b60; [SUMMARY](./phases/82-builder-dx-prerequisites/82-01-SUMMARY.md))
- [x] 82-02-PLAN.md — Reference test `tests/in_process_handler_pattern.rs`
- [x] 82-03-PLAN.md — Book section on handler-level testing pattern

### Phase 83: Toolkit Core Lift (`pmcp-server-toolkit`)

**Goal**: A new public `crates/pmcp-server-toolkit/` crate exposes the `mcp-server-common` shape (auth, secrets, static resources, static prompts, HMAC tokens, `ToolInfo` synthesis from `[[tools]]` config, code-mode policy wiring) so any external developer can build a config-driven MCP server core without depending on `pmcp-run` internals. The three pmcp-run backend cores cut their path-deps and gain independent release cadence.
**Depends on**: Phase 82 (uses `tool_arc` / `prompt_arc`)
**Requirements**: TKIT-01, TKIT-02, TKIT-03, TKIT-04, TKIT-05, TKIT-06, TKIT-07, TKIT-08, TKIT-09, TKIT-10, TEST-02, TEST-03
**Success Criteria** (what must be TRUE):

  1. A developer can add `pmcp-server-toolkit = "<published-version>"` to their `Cargo.toml` from crates.io and import `AuthProvider`, `SecretsProvider`, `StaticResourceHandler`, `StaticPromptHandler`, HMAC token helpers, and the `[[tools]]` `ToolInfo` synthesizer from a single crate
  2. A `config.toml` matching `pmcp-run/built-in/sql-api/servers/open-images/config.toml` (or `imdb` / `msr-vtt`) parses without modification through the toolkit — `[[tools]]` entries with `[[tools.parameters]]` (type/description/required/default/min/max/max_length) and `[tools.annotations]` (read_only_hint/destructive_hint/idempotent_hint/open_world_hint/cost_hint) produce complete `ToolInfo` definitions with **zero** per-tool Rust handlers written
  3. The `[code_mode]` block (enabled, allow_writes, allow_deletes, allow_ddl, require_limit, max_limit, blocked_tables, sensitive_columns, auto_approve_levels, token_ttl_seconds, token_secret) plus `[code_mode.limits]` (max_tables_per_query, max_join_depth, max_subquery_depth) wires into `pmcp-code-mode`'s validation pipeline + `CodeExecutor` with **zero** per-server Rust glue — same surface as open-images config.toml lines 97–127
  4. Code-mode prompt body assembly combines dialect-aware schema text (CONN-04, from Phase 84) with `[[database.tables]]` curated table descriptions so the LLM is seeded with both raw DDL and semantic hints
  5. All three pmcp-run backend cores (`mcp-sql-server-core`, `mcp-graphql-server-core`, `mcp-openapi-server-core`) replace their `pmcp-run/built-in/shared/` path-deps with versioned crates.io `pmcp-server-toolkit` deps and continue to pass their existing tests unchanged

**Plans:** 9/9 plans complete

Plans:
**Wave 1**

- [x] 83-01-PLAN.md — Crate scaffold + workspace insertion + module skeleton + reference fixtures (TKIT-01) (Wave 1)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 83-02-PLAN.md — Lift auth.rs + secrets.rs from mcp-server-common (TKIT-02, TKIT-03) (Wave 2)
- [x] 83-03-PLAN.md — Lift resources.rs + prompts.rs from mcp-server-common (TKIT-04, TKIT-05) (Wave 2)
- [x] 83-04-PLAN.md — ServerConfig parser + REF-01 superset integration test (TKIT-01) (Wave 2)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 83-05-PLAN.md — [[tools]] → ToolInfo synthesizer with property test (TKIT-07, TEST-02) (Wave 3)
- [x] 83-06-PLAN.md — Code-mode wiring + HMAC re-exports + policy integration test (TKIT-06, TKIT-09) (Wave 3)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 83-07-PLAN.md — SqlConnector trait stub + Dialect + assemble_code_mode_prompt (TKIT-10, TEST-02) (Wave 4)

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 83-08-PLAN.md — ServerBuilderExt + backend-core smoke test + ALWAYS example (TKIT-08, TEST-03) (Wave 5)

**Wave 6** *(blocked on Wave 5 completion)*

- [x] 83-09-PLAN.md — Fuzz target + contract YAML + shim diff + migration guide + publish-gate (TKIT-01, TKIT-08, TEST-02) (Wave 6)

### Phase 84: SQL Connectors (Postgres / MySQL / Athena / SQLite)

**Goal**: A toolkit consumer picks one or more backend crates (`pmcp-toolkit-postgres`, `pmcp-toolkit-mysql`, `pmcp-toolkit-athena`, or the `sqlite` feature flag) and gets a complete `SqlConnector` impl driven entirely by pure-Rust drivers (`tokio-postgres`, `sqlx`, `aws-sdk-athena`, bundled `rusqlite`) — no Docker, no testcontainers, Lambda-deployable as a pure-Rust binary.
**Depends on**: Phase 83 (`SqlConnector` trait lives in toolkit core)
**Requirements**: CONN-01, CONN-02, CONN-03, CONN-04, CONN-05, CONN-06, CONN-07, CONN-08, TEST-01, TEST-07
**Success Criteria** (what must be TRUE):

  1. A `SqlConnector` trait with exactly **three** methods (`dialect()`, `execute(query, params)`, `schema_text()`) is in toolkit core, and `schema_text()` optionally folds in per-table descriptions from `[[database.tables]]` config entries so curated descriptions reach the code-mode prompt
  2. Canonical `:name` placeholders in a single `config.toml` translate correctly to dialect-specific placeholder forms (`$1` for Postgres, `?` for MySQL, `?` for Athena, `:name` for SQLite) via the `translate_placeholders` free helper — verified by property tests
  3. `build_code_mode_prompt(connector)` assembles a dialect-aware code-mode bootstrap prompt body whose schema section comes from the connector's `schema_text()` — verified for all four dialects
  4. Each per-backend crate (Postgres / MySQL / Athena) is publishable to crates.io and integration-tested against an **authentic in-process mock** for that backend (Postgres `$1`+`information_schema`, MySQL `?`+`information_schema`, Athena `?`+Glue catalog) — no `testcontainers`, no Docker; SQLite tested against a real in-memory `rusqlite` DB
  5. A fuzz target on the `config.toml` parser (extending Phase 77's `pmcp_config_toml_parser`) confirms malformed config never panics — runtime stress in CI/nightly per the same disposition as Phase 77 Plan 08

**Plans**: 9 plans

- [x] 84-00-PLAN.md — Wave 0 scaffolding: 3 per-backend crate skeletons + translate.rs shell + property-test scaffold (RED) + fuzz corpus seed
- [x] 84-01-PLAN.md — Extend SqlConnector trait to 3 methods (execute) + 4 ConnectorError variants
- [x] 84-02-PLAN.md — translate_placeholders SqlWalker state machine + 5 property invariants (RED→GREEN→REFACTOR)
- [x] 84-03-PLAN.md — build_code_mode_prompt alias + DatabaseSection.url field + synthesizer connector threading + widget_meta flip
- [x] 84-04-PLAN.md — SqliteConnector promotion + sqlite_minimal Shape C example
- [x] 84-05-PLAN.md — pmcp-toolkit-postgres (deadpool-postgres + PgParam ToSql + PostgresMock + 4 D-13 tests)
- [x] 84-06-PLAN.md — pmcp-toolkit-mysql (sqlx pure-Rust TLS + MysqlMock + 4 D-13 tests)
- [x] 84-07-PLAN.md — pmcp-toolkit-athena (aws-sdk-athena NO Glue + polling + AthenaMock + 4 D-13 tests)
- [x] 84-08-PLAN.md — Fuzz corpus extension (3 backend seeds) + CLAUDE.md publish-order + REQUIREMENTS closure + verification sweep

### Phase 85: Shape A Pure-Config Binary + Reference Parity

**Goal**: A non-developer can take any of the existing `pmcp-run/built-in/sql-api/servers/*/config.toml` files unchanged, run `pmcp-sql-server --config <file> --schema <file>`, and get a live MCP server with the same tools, same code-mode policy, and same observable behavior as the production pmcp-run server — proving the toolkit lift end-to-end.
**Depends on**: Phase 84 (Shape A binary needs at least one backend connector to run against)
**Requirements**: SHAP-A-01, REF-01, REF-02
**Success Criteria** (what must be TRUE):

  1. Running `pmcp-sql-server --config pmcp-run/built-in/sql-api/servers/open-images/config.toml --schema <schema-file>` (or `imdb` / `msr-vtt`) produces a running MCP server with **zero** Rust written by the user
  2. The toolkit's `config.toml` schema is a **superset** of the existing pmcp-run sql-api server configs — any of the three reference servers' configs parse cleanly, additive new keys are allowed, **renames are not**
  3. The reproduced server responds to `tools/list`, `tools/call` for every `[[tools]]` entry, **and** the code-mode pair (`validate_code` / `execute_code`) with policy enforcement matching the production server's behavior
  4. Replaying a representative subset of `pmcp-run/built-in/sql-api/reference/scenarios/` against both the original pmcp-run server and the Shape A reproduction yields **result parity** on the asserted scenarios

**Plans**: 6 plans (4 waves)

Plans:
**Wave 1**

- [x] 85-01-PLAN.md — REF-01 superset config fields (file_path / is_reference / [shared_policy_store]) + ${VAR} expansion gate [wave 1]
- [x] 85-03-PLAN.md — pmcp-sql-server crate skeleton + vendored Chinook DDL/scenarios/config fixtures [wave 1]

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 85-02-PLAN.md — Real code-mode tool registration + SqlCodeExecutor adapter + file-based prompt seam [wave 2]
- [x] 85-04-PLAN.md — clap CLI + [database] type → connector dispatch [wave 2]

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 85-05-PLAN.md — Server assembly + streamable-HTTP serving + SC-1 lazy-startup + SC-2 superset-parse tests [wave 3]

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 85-06-PLAN.md — Chinook parity replay (REF-02/SC-3/SC-4) + Shape C example + doctests + publish-order + fuzz seed [wave 4]

**Gap closure** *(SC-3 reopened by code review — see 85-VERIFICATION.md Gaps 1-3 + 85-REVIEW.md)*

- [x] 85-07-PLAN.md — Gap 1: enforce `require_limit` (add+map `sql_require_limit`; no-LIMIT read rejected independent of `max_limit`) (SHAP-A-01) [gap, wave 1]
- [x] 85-09-PLAN.md — Gap 3: synthesize `code-mode://instructions` + `code-mode://policies` resources during assembly + prompt-body content assertion (REF-02) [gap, wave 1]
- [x] 85-08-PLAN.md — Gap 2: make policy-rejection scenarios individually gating in `parity_chinook.rs` (per-step assertion, fixtures unchanged) (REF-02, SHAP-A-01) [gap, wave 2, depends 85-07/85-09]
- [x] 85-10-PLAN.md — Secondary fixes: execute_code variables, null-default bind, cached pipeline, JoinError exit, sqlite `database` form, empty AWS_REGION/token_secret (SHAP-A-01, REF-02) [gap, wave 2, depends 85-07/85-09]

### Phase 86: Shapes B/C/D — Scaffold, Library Example, Deploy

**Goal**: A developer can choose any of three ergonomics levels for non-pure-config use cases — scaffold a starter project with `cargo pmcp new --kind sql-server` (Shape B), wire a ≤15-line `main.rs` library use (Shape C), or `cargo pmcp deploy` a config-only server to pmcp.run as a hosted target (Shape D) — and Phase 77's `cargo pmcp configure` target system accommodates each without breaking changes.
**Depends on**: Phase 85 (Shape A proves the binary surface before scaffolding spawns clones of it)
**Requirements**: SHAP-B-01, SHAP-C-01, SHAP-D-01, TEST-05, TEST-06
**Success Criteria** (what must be TRUE):

  1. `cargo pmcp new --kind sql-server` scaffolds a starter project containing `Cargo.toml` (pinned `pmcp-server-toolkit` + chosen backend dep), `main.rs` (Shape C wiring in ≤15 lines), and `config.toml` (commented template); running `cargo run` against an embedded SQLite gets `tools/list` + at least one `tools/call` working — verified end-to-end by an integration test in a tempdir
  2. A runnable example under `examples/` proves Shape C library use: a complete MCP server in **≤15 lines** of `main.rs` (toolkit + a chosen backend connector)
  3. `cargo pmcp deploy` packages a config-only server as a pure-Rust Lambda binary and deploys it to pmcp.run; the Phase 77 `cargo pmcp configure` target system handles config-only-server targets with **no breaking changes** to existing target variants
  4. A deploy integration test exercises at least one config-only-server deploy against a mock or real pmcp.run target and confirms the post-deploy lifecycle (Phase 79 `check` + `conformance` + `apps` verifier) runs cleanly

**Plans**: 6 plans

Plans:
**Wave 1**

- [x] 86-01-PLAN.md — execute_batch bootstrap helper + `http` feature forward (pmcp/streamable-http) + asset/db-path resolver (demo_db_path, /var/task vs /tmp) + CONCRETE single-crate-deploy spike [wave 1]

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 86-02-PLAN.md — Shape C: ≤15-line serving example (toolkit + SQLite connector) + spawn-poll integration test + body-count assertion [wave 2]

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 86-03-PLAN.md — Shape B: `new --kind sql-server` single-crate emitter (emits the Shape C wiring) + scoped README docs [wave 3]

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 86-04-PLAN.md — TEST-05: tempdir scaffold -> patch-deps -> real cargo run -> poll -> tools/list + tools/call [wave 4]
- [x] 86-05-PLAN.md — Shape D: detection seam + single-crate Lambda build (builder.rs) + deploy.toml [assets]/pmcp-run + env-ref secret posture + packaging/D-10 tests [wave 4]

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 86-06-PLAN.md — TEST-06: env-gated real pmcp.run deploy + Phase 79 check/conformance/apps lifecycle (D-11) [wave 5]

### Phase 87: Type 2 Authoring Skills MCP Server (`pmcp-config-helper`)

**Goal**: A non-developer using a SEP-2640-capable MCP client gets canonical `config.toml` authoring guidance — root SKILL.md + per-backend references + at least one worked example — served by the `pmcp-config-helper` MCP server, with the SEP-2640 dual-surface invariant (prompt body byte-equals SKILL.md) and §9 list-exclusion compliance asserted in-binary. Coding agents writing Rust against the toolkit pick up the same canonical idioms via Type 1 `ai-agents/` updates.
**Depends on**: Phase 83 (Skills bundle teaches the toolkit's config shape; needs the public toolkit on crates.io to exist)
**Requirements**: SKLL-01, SKLL-02, SKLL-03, SKLL-04, SKLL-05, SKLL-06, SKLL-07, TEST-04
**Success Criteria** (what must be TRUE):

  1. A SEP-2640-capable MCP client connecting to the `pmcp-config-helper` binary sees the root SKILL.md (covering curated-tool pareto, secrets refs, auth surface, code-mode opt-in) and can `resources/read` per-backend references (`references/postgres.md`, `references/mysql.md`, `references/athena.md`, `references/sqlite.md`) plus at least one worked example bundle (`config.toml` + `schema.sql`)
  2. **Dual-surface invariant** — `prompts/get` body for the bootstrap prompt is **byte-equal** to the root SKILL.md content, asserted by an in-binary integration test (spike 002's invariant)
  3. **SEP-2640 §9 compliance** — supporting files (per-backend references, worked example bundle) are served via `resources/read` but **MUST NOT** appear in `resources/list`, asserted by an integration test against a representative client
  4. The `pmcp-config-helper` crate is publishable to crates.io with a `pmcp-config-helper` binary that runs the server with default skills bundled — no extra setup required
  5. Type 1 build-time skills in `ai-agents/` are updated with toolkit-authoring patterns (config DSL, connector trait usage, secrets binding) so coding agents writing Rust against `pmcp-server-toolkit` pick up canonical idioms from their dev environment

**Plans**: TBD

### Phase 88: Dogfood — `crates/pmcp-server` on Toolkit

**Goal**: The SDK's own dev-tools MCP server (`crates/pmcp-server`) is rewritten on top of `pmcp-server-toolkit` with at least one config-driven tool surface, demonstrating the toolkit's reach. Downstream users see **no functional regression** — the rewritten server passes the existing test suite (or a documented superset) unchanged.
**Depends on**: Phase 83 (uses the public toolkit), Phase 84 (uses at least one connector for the config-driven tool surface)
**Requirements**: DOGF-01, DOGF-02
**Success Criteria** (what must be TRUE):

  1. `crates/pmcp-server` is rewritten on top of `pmcp-server-toolkit` and exposes at least one tool defined via `[[tools]]` config rather than a hand-written Rust handler
  2. The existing `pmcp-server` test suite (or a documented superset) passes unchanged — **no functional regression** for any current downstream user
  3. The dogfood rewrite surfaces and resolves any toolkit DX paper-cuts (logged as fold-back fixes into Phase 83 / 84 follow-ups before milestone close) before the toolkit's first published version

**Plans**: TBD

### Phase 89: Documentation, Migration Guide & Examples Index

**Goal**: A developer landing on the PMCP repo or docs.rs sees config-first positioning ("build production MCP servers from config alone"), can follow a book chapter through the four DX shapes + per-backend recipes + deployment, can work through a hands-on course tutorial from `cargo pmcp new --kind sql-server` to a deployed pmcp.run server, and can find a one-page recipe for moving an existing pmcp-run sql-api server author from in-tree path-deps to the public toolkit.
**Depends on**: Phase 86 (all four shapes shipped), Phase 87 (Type 2 authoring server shipped — book + course mention it), Phase 88 (dogfood validates the docs' usage claims)
**Requirements**: DOCS-01, DOCS-02, DOCS-03, DOCS-04, DOCS-05, REF-03
**Success Criteria** (what must be TRUE):

  1. A new book chapter in `pmcp-book/src/` covers config-only MCP servers — overview, the four shapes, per-backend recipes (Postgres / MySQL / Athena / SQLite), deployment to pmcp.run
  2. A new course tutorial in `pmcp-course/src/` walks a hands-on path from `cargo pmcp new --kind sql-server` → local `cargo run` → `cargo pmcp deploy` → live pmcp.run server
  3. The book chapter includes a **migration note** (REF-03) — one-page recipe showing how a pmcp-run SQL-API server author swaps the path-dep for the public toolkit, drops the duplicate domain crates, and regenerates
  4. The PMCP README and `CRATE-README.md` lead with config-first positioning ("build production MCP servers from config alone"), with the four shapes prominently introduced
  5. The `examples/README.md` index gains config-only entries (Shape A binary use, Shape C library use); the `cargo-pmcp` README documents `new --kind sql-server` scaffolding and `deploy` for config-only server targets

**Plans**: TBD

### Phase 90: OpenAPI Built-In Server (`pmcp-openapi-server`)

**Goal**: Deliver a config-driven **OpenAPI** MCP server that mirrors the completed SQL toolkit (Shape A binary `pmcp-sql-server`, Phases 83–86): a non-developer points a binary at a `config.toml` + an OpenAPI spec and gets a live MCP server — curated operation→tool mappings for the common ~20%, Code Mode (the existing `openapi-code-mode` feature in `pmcp-code-mode`) for the long-tail ~80% — with **zero Rust written**. The backend-agnostic toolkit (Phase 83) and the Shape A / scaffold / deploy patterns (Phases 85–86) are reused; only an OpenAPI connector model, the operation→tool config mapping, the `pmcp-openapi-server` binary, the `cargo pmcp new --kind openapi-server` scaffold, and docs are new.
**Depends on**: Phase 83 (backend-agnostic toolkit core), Phase 85 (Shape A binary pattern), Phase 86 (scaffold + deploy). Reuses the existing `openapi-code-mode` feature in `pmcp-code-mode`.
**Requirements**: OAPI-01 (HttpConnector trait), OAPI-02a (single-call tool synth), OAPI-02b (script tools — D-01), OAPI-03 (5-variant outgoing auth — D-05), OAPI-04 (openapiv3 --spec parser — D-03), OAPI-05 (HttpCodeExecutor seam), OAPI-06 (Shape A binary), OAPI-07 (--kind openapi-server scaffold + deploy), OAPI-08 (london-tube wiremock parity — D-04), OAPI-09 (docs in three shapes), OAPI-10 (generalize code_mode wiring to Arc<dyn CodeExecutor> + the one-engine parity proof — D-02). New scope added 2026-05-29; refined by the RESEARCH pass.
**Reference to lift from (CONFIRMED)**: `~/Development/mcp/sdk/pmcp-run/built-in/openapi-api` — the OpenAPI sibling of the `sql-api` reference the SQL toolkit (Phases 83–86) was lifted from. This is the source-of-truth for Phase 90, exactly as `sql-api` was for SQL. Structure:

  - **`crates/mcp-openapi-server-core`** — the core to lift (analog of `mcp-sql-server-core`). Modules: `auth`, `code_mode`, `config`, `http`, `schema`, `secrets`, `templates`, `tools`, `pmcp_server`, `lambda`. Builds on `pmcp` (workspace), `openapiv3` (spec parse), `reqwest 0.13` (rustls), `serde_yaml`, and `pmcp-code-mode 0.4.0` with the **`js-runtime`** feature (long-tail = validated JS calling the API). Currently uses `shared/mcp-server-common` + `shared/mcp-lambda-proxy` path-deps to be replaced by the public `pmcp-server-toolkit` (the REF-style lift).
  - **Config shape** (analog of SQL's `[database]`): `[backend] base_url` + `[backend.auth]` (`type = "bearer"`, `token = "${ENV}"`, `required`) + `[backend.http]` (timeout/retries/backoff) + `[[tools]]` mapping an operation via `path` / `method` / `base_url` (the operation→tool analog of SQL's `sql=`) + `[secrets]` + `[metadata]` + `[observability]`.
  - **Instance configs (parity fixtures)**: `instances/{lichess,london-tube,dhl,rest-admin}.toml` and `servers/{lichess,london-tube,dhl,aws-cloudwatch,aws-billing,rest-admin}/`. `lichess` and `london-tube` are public / no-auth-friendly → best demo + parity candidates.
  - **Design docs to mine**: `OPENAPI_CODE_MODE_DESIGN.md`, `OPENAPI_CODE_MODE_POLICY_DESIGN.md`, `OPENAPI_CODE_MODE_ACCESS_CONTROL.md`, `OPENAPI_SCRIPT_TOOLS.md`, `BUILTIN_SERVER_ARCHITECTURE.md`, `DEPLOYMENT.md`.

**Scope (mirrors the SQL lift; confirm details in RESEARCH)**:

  - Lift `mcp-openapi-server-core`'s reusable glue into `pmcp-server-toolkit` (auth/secrets/config/code-mode wiring is largely shared; the NEW backend piece is an **HTTP/OpenAPI connector** analogous to `SqlConnector` at `crates/pmcp-server-toolkit/src/sql/mod.rs`).
  - `[backend]`-driven config: curated `[[tools]]` → OpenAPI operations (`path`/`method`/`base_url`), bearer/`${ENV}` auth, retries/timeouts.
  - A `pmcp-openapi-server` Shape A binary (`--config` + `--spec`), mirroring `pmcp-sql-server`.
  - A `cargo pmcp new --kind openapi-server` scaffold, mirroring `--kind sql-server`.
  - Reuse `pmcp-code-mode`'s `js-runtime` / `openapi-code-mode` feature for the Code Mode long-tail path.
  - REF-style parity: a reference instance (e.g. `lichess`) reproduces the pmcp-run server's tools + behavior unchanged.
  - Docs in three shapes (crate README + `pmcp-book` chapter + `pmcp-course` chapter), matching the SQL docs.

**Open questions for RESEARCH**: how much of `mcp-openapi-server-core` is already covered by the backend-agnostic toolkit (Phase 83) vs genuinely new; the exact HTTP-connector trait shape; auth models beyond bearer (apiKey/basic/oauth) in the instance configs; spec-source handling (`--spec` file vs inline `[[tools]]`-only); and whether code-mode's `js-runtime` needs toolkit-side wiring like the SQL `executor_from_config` seam.

**Plans**: 9 plans in 7 waves + 4 gap-closure plans in 2 waves (planned 2026-05-29; replanned with cross-AI review feedback; gap-closure planned 2026-05-29 from VERIFICATION gaps_found 9/11 + REVIEW WR-02/03/04)

Plans:
**Wave 1**

- [x] 90-01-PLAN.md — HttpConnector trait + reqwest client + 5-variant outgoing auth + http feature (OAPI-01/03)
- [x] 90-02-PLAN.md — additive [backend]/[backend.auth]/[backend.http] + ToolDecl two-kind fields on ServerConfig (D-06, OAPI-02a/03)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 90-03-PLAN.md — openapiv3 --spec parser (optional at runtime) + single-call tool synthesizer (OAPI-04/02a)
- [x] 90-04-PLAN.md — HttpCodeExecutor seam + generalize code_mode_tools_from_executor to Arc<dyn CodeExecutor>+flavor (OAPI-05/10)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 90-05-PLAN.md — ScriptToolHandler (one engine, no token cycle) + the D-02 engine-parity proof (OAPI-02b/10)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 90-06-PLAN.md — pmcp-openapi-server Shape A binary (cli/dispatch/assemble/lib, streamable HTTP, spec optional) (OAPI-06)

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 90-07-PLAN.md — london-tube wiremock parity replay + env-gated live test (OAPI-08, D-04)
- [x] 90-08-PLAN.md — cargo pmcp new --kind openapi-server scaffold + deploy parity + scoped README (OAPI-07)

**Wave 6** *(blocked on Wave 5 completion)*

- [x] 90-09-PLAN.md — docs in three shapes (crate README + book + course chapters) (OAPI-09)

**Gap-Closure Wave 1** *(closing VERIFICATION gaps + REVIEW warnings; planned 2026-05-29)*

- [x] 90-10-PLAN.md — oauth_passthrough per-request token made live at runtime: toolkit-resident request_executor_from_extra seam called by ScriptToolHandler + ExecuteCodeHandler; dispatch installs OAuthPassthroughAuth; e2e wiremock proof (closes VERIFICATION truths #3/#8, WR-01; OAPI-03/05)
- [x] 90-11-PLAN.md — cross-variant ${VAR}/env:VAR secret resolution chokepoint applied to bearer/basic/oauth2 credential fields (closes altitude finding; OAPI-03)
- [x] 90-12-PLAN.md — backend.base_url non-empty validation + oauth_passthrough trust-boundary docs (closes WR-02/WR-04; OAPI-03)

**Gap-Closure Wave 2** *(blocked on 90-10 — shared code_mode.rs)*

- [x] 90-13-PLAN.md — reject non-scalar path/query/header params with a value-redacted error instead of silent JSON-stringification (closes WR-03; OAPI-02a/05)

## Progress — v2.2 Milestone

**Execution order:** Phase 82 → Phase 83 → Phase 84 → Phase 85 → Phase 86 (Shapes B/C/D) and Phase 87 (Skills) in parallel after 83 lands → Phase 88 (dogfood) → Phase 89 (docs). Phase 90 (OpenAPI built-in, added 2026-05-29) is independent of 87–89 — it reuses the toolkit core (83) and Shape A/scaffold patterns (85–86), so it can proceed in parallel with the remaining SQL-milestone phases.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 82. Builder DX Prerequisites | 3/3 | Complete   | 2026-05-18 |
| 83. Toolkit Core Lift | 9/9 | Complete   | 2026-05-18 |
| 84. SQL Connectors | 9/9 | Complete   | 2026-05-26 |
| 85. Shape A + Reference Parity | 10/10 | Complete    | 2026-05-27 |
| 86. Shapes B/C/D | 6/6 | Complete    | 2026-05-27 |
| 87. Type 2 Authoring Skills Server | 0/? | Not started | - |
| 88. Dogfood `pmcp-server` | 0/? | Not started | - |
| 89. Documentation & Migration | 0/? | Not started | - |
| 90. OpenAPI Built-In Server | 13/13 | Complete   | 2026-05-30 |

### Phase 90.2: OpenAPI Built-In Server — Advanced Example (Contoso M365: OAuth passthrough + Excel-over-Graph) (INSERTED)

**Goal:** Ship a second, advanced OpenAPI showcase that demonstrates enterprise OAuth and business-data access, distinct from London Tube's `api_key` (90.1). Vehicle: Microsoft Graph / M365 for a fictional org "Contoso", READ-ONLY. The headline narrative: *keep your existing Excel files, connect them to AI via MCP* — and the business analyst curates the relevant slice of a huge API rather than dumping full Graph metadata. Auth = `oauth_passthrough`: an org admin consents once to a bounded scope (the ceiling of what the server may request), and the signed-in user's forwarded token (from their MCP client, e.g. ChatGPT) governs per-file access — the server holds no standing credentials and can only ever act as the calling user.

Concrete shape: a demo Excel workbook in SharePoint/OneDrive with two sheets — **Customers** and **Orders** (orders belong to customers). Two explicit MCP tools over the Graph Excel range-read API — `get_customer` and `get_customer_orders` — with everything richer left to Code Mode (e.g. "customers who bought more than 100 in the last 3 months"). A curated/trimmed Graph OpenAPI spec (~3–4 read-only ops: list SharePoint files, file content, Excel worksheet range read), NOT the full metadata.

This is config + curated spec + fixture + docs, NOT a feature build: `AuthConfig::OAuthPassthrough` (`crates/pmcp-server-toolkit/src/http/auth.rs:123`) and the full passthrough chain (`TokenCaptureAuthProvider → AuthContext → HttpCodeExecutor::with_inbound_token → outbound forward`, `crates/pmcp-openapi-server/src/assemble.rs:22-31,93`) already ship from Phase 90 (Plan 90-10). Mirror the London Tube structure (90.1): fixture + pointable example + book/course chapters + offline `parity_replay` (wiremock asserts the forwarded `Authorization: Bearer` reached the Graph backend), plus an `#[ignore]`+env-gated live test like `parity_live_tfl`. NOTE: `@odata.nextLink` pagination is NOT handled by the connector today and is NOT needed for a single demo workbook — explicitly out of scope (avoids turning this into a feature build).

**Decision:** READ-ONLY (no Excel write / workbook-session path); `oauth_passthrough` is the hero auth path (app-only client-credentials only mentioned as a documented contrast, if at all).
**SQL/OpenAPI parallels (mirror these):** 90.1 london-tube fixture/example/chapters/`parity_replay.rs`; `crates/pmcp-sql-server/tests/fixtures/reference-config.toml`.
**Requirements**: P902-SPEC, P902-CONFIG, P902-FIXTURE, P902-PARITY, P902-CODEMODE, P902-EXAMPLE, P902-DOCS-BOOK, P902-DOCS-COURSE (locked via SPEC.md)
**Depends on:** Phase 90
**Plans:** 4/4 plans complete

Plans:

- [x] 90.2-01-PLAN.md — Contoso M365 fixtures: CANONICAL workbook dataset (contoso-m365-workbook.json) + oauth_passthrough config + two customer_id-keyed script tools + curated Graph range-read spec + replay scenario (P902-SPEC, P902-CONFIG, P902-FIXTURE)
- [x] 90.2-02-PLAN.md — Offline parity test (forwarded-bearer passthrough proof) + pointable examples/contoso-m365.toml + build-only example (P902-PARITY, P902-EXAMPLE)
- [x] 90.2-03-PLAN.md — Deterministic headline Code Mode Rust test (>100-in-3-months, pinned reference date) (P902-CODEMODE)
- [x] 90.2-04-PLAN.md — Dedicated book + course chapters (governance + Excel narrative) wired into both SUMMARYs (P902-DOCS-BOOK, P902-DOCS-COURSE)

### Phase 90.1: OpenAPI Built-In Server — Examples & Article Parity (INSERTED)

**Goal:** Bring the OpenAPI built-in server's examples and documentation to full parity with the SQL server. Scope (locked via progress routing):

1. Enrich `crates/pmcp-openapi-server/tests/fixtures/london-tube.toml` from a thin 114-line test fixture into the full annotated showcase reference instance — the OpenAPI equivalent of SQL's `chinook` `reference-config.toml`: add `[[resources]]` (TfL API schema docs + sample request/response payloads + example scripts), `[[prompts]]` (start-code-mode), and full per-tool `[tools.annotations]` (read_only_hint, idempotent_hint, cost_hint, auth hints).
2. Add a runnable example config users can point at.
3. Deepen `pmcp-book/src/openapi-built-in-server.md` and `pmcp-course/src/openapi-built-in-server.md` with a resources/prompts config-walkthrough section — the one section SQL's chapters have that OpenAPI skips (chapter *structure* is already at parity from Phase 90-09; this adds the missing config-depth section).
4. Add a replay/parity test covering the enriched fixture (ALWAYS requirements: parity test + example demonstration).

**Decision:** Keep london-tube as the single showcase; do NOT add a second real-world API.
**SQL parallels (mirror these):** `crates/pmcp-sql-server/tests/fixtures/reference-config.toml` (670 lines), `chinook.ddl`, `pmcp-book/src/ch12-10-config-driven-sql-servers.md`, `pmcp-course/src/part3-deployment/ch08-5-config-driven-sql-server.md`.
**Requirements**: P901-FIXTURE, P901-EXAMPLE, P901-DOCS-BOOK, P901-DOCS-COURSE, P901-PARITY
**Depends on:** Phase 90
**Plans:** 3/3 plans complete

Plans:

- [x] 90.1-01-PLAN.md — Enrich london-tube.toml (resources + prompt + full annotations) + fixture-validity asserts + build-only example (P901-FIXTURE, P901-EXAMPLE)
- [x] 90.1-02-PLAN.md — Add Resources & Prompts config-walkthrough section to book + course OpenAPI chapters (P901-DOCS-BOOK, P901-DOCS-COURSE)
- [x] 90.1-03-PLAN.md — Add list_resources + list_prompts replay steps to london-tube-scenarios.yaml (P901-PARITY)

## v2.3 Excel-as-Configuration MCP Servers (governed Excel CodeLanguage) (In Progress)

**Milestone Goal:** Extract the proven Excel-workbook → MCP-server compiler from the `towelrads-quote-pricing` lighthouse (its milestone v0.5.0, phases 7–14, all green — golden quote penny-reconciled to ±£0.01) into the PMCP SDK as a third "governed Excel" CodeLanguage alongside the v2.2 SQL and OpenAPI toolkits. **Compile, never interpret:** the workbook is simultaneously the specification (formula DAG), the test oracle (cached cell values become assertions), and the output template. Any project can compile a governed Excel workbook into a tested, versioned, deterministic MCP server. Generalize the known lighthouse debt (RFC §5) — do not copy it.

**Source of truth:** RFC `docs/sdk-issue-excel-workbook-compiler-extraction.md` + `.planning/research/` (STACK / FEATURES / ARCHITECTURE / PITFALLS / SUMMARY, all HIGH confidence, researched 2026-06-09). Reference implementation: the lighthouse `crates/workbook-runtime/`, `crates/workbook-compiler/`, and `crates/quote-pricing-server/src/workbook/` (the served layer is already ~95% workbook-agnostic).

**Load-bearing invariants encoded across phases:**

- **Purity invariant (Pitfall 1):** the Excel reader (`umya` / `quick-xml`) must NEVER enter the served-binary dependency tree. The served path links only `pmcp-workbook-runtime`; the reader lives only in `pmcp-workbook-compiler` (consumed only by `cargo-pmcp`). A `cargo tree` + `cargo-deny [bans]` purity gate stands up in Phase 91 (with the runtime, before any `umya` code lands) and is re-asserted in every later phase that touches the served tree. The writer (`rust_xlsxwriter`, pulls permitted `zip`) IS positively asserted present.
- **Dependency-forced ordering:** runtime ← compiler ← CLI; runtime ← served-tool toolkit module ← Shape A binary ← Shape B scaffold. The served layer requires the bundle contract; the bundle contract requires the compiler; the compiler requires the dialect + runtime. Freeze the bundle contract from the consumer side (Phase 92) BEFORE the compiler is re-cut (Phase 93).
- **§5 generalization, not copy:** kill hardcoded `build_reference_manifest` (manifest fully synth-driven), fix promote-path bugs CR-01 (demotion asymmetry) / CR-02 (version overwrite) / WR-01 (enum-input tiering), handle umya fabricated-provenance — all in the compiler-owning phase (93).
- **Second-workbook test (WBEX-01) is the generalization gate:** it lands in Phase 96, after the compiler + served layer are manifest-driven, and its success proves the §5 manifest-driven fix actually generalized (no per-workbook Rust, no privileged single output).
- **Mirror the v2.2 toolkit pattern:** `pmcp-server-toolkit` feature module + per-source crates + Shape A binary / Shape B scaffold, same `TypedToolWithOutput` → `outputSchema` → `structuredContent` discipline.
- **Explicitly NOT touched:** `pmcp-code-mode` (the untrusted long-tail path). A compiled workbook is curated config trusted by the promote gate + BA curation, not a runtime token. The two CodeLanguages coexist.

### v2.3 Phase Summary

- [x] **Phase 91: Workbook Runtime + Purity Gate + Dialect Spec** — Port the reader-free `pmcp-workbook-runtime` leaf (owned IR/model types, deterministic evaluator, writer-only `.xlsx` renderer) and stand up the `cargo tree` + `cargo-deny` purity gate on day one; ship the SDK-owned versioned dialect spec + linter (completed 2026-06-10)
- [x] **Phase 92: BundleSource + Served-Tool Toolkit Module** — Freeze the bundle contract from the consumer side: `BundleSource` trait (local-dir + embedded) + the generic, fully manifest-driven `workbook` feature module in `pmcp-server-toolkit` (all five tools, fail-closed validation, boot integrity gate) against a test bundle (verification: gaps found 2026-06-10) (completed 2026-06-11)
- [x] **Phase 93: Workbook Compiler + §5 Generalization Fixes + Promote Gate** — Port `pmcp-workbook-compiler` (umya-isolated offline pipeline), do the §5 fixes here (manifest-driven emit, CR-01/CR-02/WR-01, umya fabricated-provenance), and ship the change-class + golden-corpus promote gate with the `--accept` approval flow (completed 2026-06-13)
- [x] **Phase 94: CLI Subcommands + `pmcp.toml`** — `cargo pmcp compile-workbook` / `lint-workbook` / `emit-bundle` thin shells over the compiler, the gated `--accept --approver --effective-date` flow, and a project-level `pmcp.toml` mapping workbooks → bundle IDs (kills single-workbook assumptions) (completed 2026-06-14)
- [x] **Phase 95: Shape A Binary `pmcp-workbook-server`** — A pure-config binary that stands up a live MCP server from a compiled bundle alone (no user Rust), mirroring `pmcp-sql-server` field-for-field (completed 2026-06-14)
- [x] **Phase 96: Shape B Scaffold + Dialect-Version Declaration + Generalization Validation** — `cargo pmcp new --kind workbook-server` scaffold (Shape B), workbooks declare the dialect version they target, and the second-workbook + Excel-quirk-corpus generalization gates prove the manifest is truly synth-driven (completed 2026-06-15)

## Phase Details — v2.3 Milestone

### Phase 91: Workbook Runtime + Purity Gate + Dialect Spec

**Goal**: A reader-free `pmcp-workbook-runtime` leaf crate owns every shared model/IR type, runs a compiled workbook's IR through a deterministic evaluator and a writer-only `.xlsx` renderer, and a mechanically-provable purity gate guarantees the Excel reader can never reach the served binary — established BEFORE any `umya` code exists. The SDK also owns a versioned dialect spec + linter.
**Depends on**: Phase 90.2 (v2.2 close); pmcp core only — proves the purity boundary first (RFC §7 smallest cut)
**Requirements**: WBRT-01, WBRT-02, WBRT-03, WBRT-04, WBDL-01
**Success Criteria** (what must be TRUE):

  1. A developer can depend on `pmcp-workbook-runtime` (reader-free leaf, slot 2a) and deserialize the shared model types (`Manifest`, `CellMap`, `BundleLock`, `VersionChangelog`, IR `Cell`/`Expr`) identically to how the offline emitter produces them — serde/schemars-clean, zero reader dependency
  2. The runtime runs a compiled IR through a deterministic topo executor producing typed outputs plus per-cell derivation traces, and renders a computed workbook back to `.xlsx` via the writer-only `rust_xlsxwriter` renderer
  3. **PURITY GATE:** CI + `just purity-check` fail the build if `umya` / `quick-xml` appear in the runtime's (or any served-binary's) dependency tree, run per feature-combination (not just defaults), with a positive assertion that `rust_xlsxwriter` IS present and `zip` (writer container) is permitted — `cargo tree` assertions backed by a `cargo-deny [bans]` declaration
  4. The SDK owns a versioned dialect spec document (function whitelist + refuse-set) bound to the `WHITELIST` const by a test that fails if doc and code diverge
  5. A developer can lint a workbook against the dialect (whitelist-only, deny-by-default) and receive collect-all, located, BA-actionable findings with repair guidance

> **Note (D-02):** WBDL-03 (the running linter + `WorkbookMap` ingest) is re-mapped to **Phase 93** — it needs a real `.xlsx` via umya. Phase 91 ships only the dialect *contract* (WHITELIST + spec doc + binding test).

**Plans**: 3 plans

Plans:

- [x] 91-01-PLAN.md — Lift the reader-free `pmcp-workbook-runtime` leaf crate (IR/model types, deterministic topo executor + traces, writer-only `.xlsx` renderer, finding model + D-08 Deserialize) (WBRT-01, WBRT-02, WBRT-03)
- [x] 91-02-PLAN.md — Create `pmcp-workbook-dialect` leaf crate (flat-13 WHITELIST + DialectRules + re-exported findings) + port `docs/workbook-dialect-spec.md` + doc↔const binding test (WBDL-01)
- [x] 91-03-PLAN.md — `make purity-check` (cargo-tree per-crate/per-feature reader-absence + writer-presence) + merge-blocking CI gate + WBDL-03 → Phase 93 re-map (WBRT-04, WBDL-03)

### Phase 92: BundleSource + Served-Tool Toolkit Module

**Goal**: The compiled-bundle contract is frozen from the consumer side: a generic, fully manifest-driven `workbook` feature module in `pmcp-server-toolkit` registers all five tools against a test bundle loaded through a `BundleSource` trait, fails closed on any integrity or validation gap, and emits the same `outputSchema` → `structuredContent` discipline as the SQL/OpenAPI toolkits — with zero per-workbook Rust.
**Depends on**: Phase 91 (runtime types + purity gate)
**Requirements**: WBSV-01, WBSV-02, WBSV-03, WBSV-04, WBSV-05, WBSV-06, WBSV-07, WBSV-08, WBSV-09
**Success Criteria** (what must be TRUE):

  1. An agent can call `calculate` with typed, tier-enforced, dtype-checked, enum-gated inputs and receive ALL named outputs (`{value,unit}` each) plus a provenance stamp — no single privileged "headline" output — and `explain` / `get_manifest` / `diff_version` / `render_workbook` each return their bundle-driven projections (per-cell trace, curated manifest, hash-verified changelog, provenance-bound `workbook://` resource)
  2. Input and output schemas are projected entirely from the manifest (`additionalProperties:false`, per-column dtype/unit/meaning; mandatory non-empty `outputSchema`) — parity with the SQL/OpenAPI `TypedToolWithOutput` pattern; no per-workbook handler code
  3. Every domain failure returns a structured `isError:true` envelope in `structuredContent` (never a protocol `Err`) carrying `code`, `reason`, and self-repair fields (`allowed`/`required`/`range`) plus the provenance stamp; validation is **fail-closed** (a missing manifest role for a supplied input is an error, not an `if let Some` skip — WR-05; non-string values on enum inputs rejected — WR-02)
  4. The server recomputes the `BUNDLE.lock` combined hash-of-hashes at boot and fails closed on any tampered or mismatched artifact before serving
  5. A server loads a bundle via the `BundleSource` trait with both local-directory and embedded (`include_dir!`) implementations; S3/registry is a documented extension seam, not shipped

**Plans**: 7 plans (7 waves, strictly sequential — 92-06/92-07 close the gaps_found verification)

- [x] 92-01-PLAN.md — Runtime BundleSource trait (local-dir + embedded) + shared fail-closed BundleLoader + manifest annotations field + D-17/S-1 scrub
- [x] 92-02-PLAN.md — Synthetic tax-calc golden fixture generator + committed byte-stable golden + tamper helpers
- [x] 92-03-PLAN.md — Toolkit workbook served core: isError envelope, manifest→schema projection, fail-closed input validation, 4 handlers (calculate/explain/get_manifest/diff_version)
- [x] 92-04-PLAN.md — render_workbook: workbook:// URI codec + stateless regen-on-read resource + published URI contract doc
- [x] 92-05-PLAN.md — WorkbookBuilderExt wiring + boot-surface re-exports + workbook feature + streamable-HTTP example + integration tests + purity-gate extension
- [x] 92-06-PLAN.md — Gap closure (CR-01): drop Role::Input cells from the golden IR + seed-preserving executor literal arm + regenerate golden + non-default-input regression test (unblocks WBSV-01/02/05)
- [x] 92-07-PLAN.md — Gap closure (Blocker 2): fail-closed override role filter (WR-02) + fail-closed project_outputs (WR-04) + absent-anchor stamp gate (WR-07) (completes WBSV-06)

**UI hint**: yes

### Phase 93: Workbook Compiler + §5 Generalization Fixes + Promote Gate

**Goal**: `pmcp-workbook-compiler` ports the full offline pipeline (ingest → lint → manifest synth → formula parse → DAG compile → penny-reconcile → artifact emit → promote-time gate) with `umya` isolated to this crate, and ships the §5 generalization fixes at extraction time (not copied): a fully manifest-driven emit path, symmetric change-class classification, versioned non-overwriting bundle writes, enum-tiering correctness, umya fabricated-provenance refusal, and the change-class + golden-corpus promote gate with a BA approval flow.
**Depends on**: Phase 91 (re-exports runtime types); contract frozen by Phase 92
**Requirements**: WBCO-01, WBCO-02, WBCO-03, WBCO-04, WBCO-05, WBCO-06, WBCO-07, WBGV-01, WBGV-02, WBGV-03, WBGV-04, WBGV-05, WBGV-06, WBGV-07, WBDL-03
**Success Criteria** (what must be TRUE):

  1. The compiler ingests a `.xlsx` (umya, compiler-isolated), captures cached cell values as a trusted oracle, parses formulas + reconstructs the dependency DAG (`sheet_ir`), compiles pure cells to executable IR, and **penny-reconciles** computed values against the oracle using operand-anchored rounding (never a naïve `delta.abs()` tolerance — grep-gated), emitting the complete seven-member bundle (manifest/IR/cell_map/layout/BUNDLE.lock/evidence)
  2. The candidate semantic manifest is synthesized **fully workbook-driven** from colour/Guide/headers with BA ratification — `build_reference_manifest` is deleted from every non-test path (kills per-workbook Rust); closed JSON-Schema enums come from inline DV literals (≤10), with range/named-range sources rejected with precise reason codes
  3. **CR-01 fix:** the change-class classifier is symmetric — demotion-direction changes (Input→Constant, source/assumption flips) each produce a non-empty class routing to BlockUntilAccept/NeverAutoPromote, never silent HotReload; the strictest-policy reducer hard-blocks any assumption (yellow-cell) change; numeric drift is distinguished from semantic redefinition via a stable canonical IR sub-DAG identity hash
  4. **CR-02 fix:** promotion writes the new bundle to its own `@<next_version>` directory and never overwrites the baseline (promote-twice yields two distinct on-disk version dirs, prior baseline byte-identical, `BUNDLE.lock` version == `changelog.to_version`); the golden-corpus gate blocks any over-tolerance named-output delta unless a content-hash-fingerprinted `ApprovalRecord` covers the candidate, and a BA can record one via `--accept --approver <X> --effective-date <D>`
  5. **WR-01 fix + umya provenance:** enum inputs skip Variable-tier assignment so the default path can never seed an out-of-enum empty string (verified against the COMMITTED manifest, not the in-memory builder); the freshness gate assigns a distinct provenance class to umya-stamped (fabricated `<Application>Microsoft Excel</Application>`/`calcId`) workbooks and REFUSES them with `oracle/non-excel-app`

**Plans**: 7 plans (6 waves)

Plans:
**Wave 1**

- [x] 93-01-PLAN.md — Crate skeleton: Cargo.toml + re-export-surface lib.rs + generic compile_workbook stub + purity-gate extension (reader confined)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 93-02-PLAN.md — ingest (umya → WorkbookMap + cached oracle) + provenance (quarantined raw reader; REFUSE umya-fabricated identity, WBCO-07) + provenance fuzz target
- [x] 93-03-PLAN.md — WBDL-03 running linter + formula parser (whitelist-at-parse) + Kahn DAG + formula-parser fuzz target

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 93-04-PLAN.md — manifest synth/ratify (annotations reconciled; inline-DV enums; range-DV warning) + operand-anchored reconcile (no delta.abs)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 93-05-PLAN.md — seven-member artifact emit (bundle_id; WR-01 enum-tier skip) + symmetric change-class classifier + IR identity hash (lift WITH tests)

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 93-06-PLAN.md — promote gate: auto-derived corpus (D-09) + fingerprint-bound ApprovalRecord + accept() + CR-02 versioned non-overwriting promote

**Wave 6** *(blocked on Wave 5 completion)*

- [x] 93-07-PLAN.md — stage1 + wired generic driver + neutral tax-calc.xlsx + producer/consumer byte-identical proof + example + full quality/purity gate

### Phase 94: CLI Subcommands + `pmcp.toml`

**Goal**: The compiler's verbs become first-class `cargo pmcp` subcommands (thin shells over the Phase 93 compiler) carrying the gated BA approval flow, and a project-level `pmcp.toml` maps workbooks → bundle IDs, eliminating the lighthouse's single-workbook justfile/path assumptions so a second project can use the tooling.
**Depends on**: Phase 93 (the compiler)
**Requirements**: WBCL-01, WBCL-02, WBCL-03, WBCL-04
**Success Criteria** (what must be TRUE):

  1. A developer can run `cargo pmcp compile-workbook <wb.xlsx>` to ingest → lint → synth → parse → compile → reconcile → **gate** → write a bundle, with the gate running before any write; and `cargo pmcp lint-workbook <wb.xlsx>` runs the dialect linter standalone (non-zero exit on errors)
  2. A developer can run `cargo pmcp emit-bundle` to regenerate a bundle without the gate (dev/reference)
  3. The `--accept --approver <X> --effective-date <D>` flow records a fingerprint-bound `ApprovalRecord` and re-baselines the golden corpus through the CLI, with clear gate output stating the change class and the exact command to run
  4. A project declares workbooks → bundle IDs in a project-level `pmcp.toml` (`[[workbook.workbooks]]` source → bundle_id), and the three CLI subcommands resolve sources through it — no lighthouse paths

**Plans**: 6 plans

Plans:
**Wave 0**

- [x] 94-00-PLAN.md — library seams in pmcp-workbook-compiler: PUBLIC read_workbook_version + prepare_candidate (gated-update candidate facade) + write_gate_marker (hash-covered ungated marker channel) — exposes existing internals only (WBCL-01 gated half, WBCL-03 marker)

**Wave 1**

- [x] 94-01-PLAN.md — pmcp.toml parser (PmcpToml load/resolve/all_entries/validate) + cargo-pmcp→compiler dep edge (WBCL-04)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 94-02-PLAN.md — `workbook` subcommand group + main.rs wiring + `lint` handler with text/json + errors-fail exit codes (WBCL-02, D-04/D-09/D-10)

**Wave 3** *(blocked on Wave 0 + Wave 2 completion)*

- [x] 94-03-PLAN.md — `compile` handler: seed/gated lane (prepare_candidate→gate::gate→block-or-promote), gate-before-write, mandatory --approver, --accept flow, compile-all (WBCL-01/04, D-06/D-07)
- [x] 94-04-PLAN.md — `emit` handler: ungated bundle + loud banner + HASH-COVERED evidence gated:false marker via write_gate_marker (WBCL-03, D-08)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 94-05-PLAN.md — end-to-end CLI integration test (incl. hash-covered emit marker) + purity-gate confirmation + runnable example (WBCL-01/02/03/04)

### Phase 95: Shape A Binary `pmcp-workbook-server`

**Goal**: A `pmcp-workbook-server` pure-config binary stands up a live MCP server from a compiled bundle alone, with no user Rust — mirroring `pmcp-sql-server` field-for-field (lib `run`/`serve` + thin `main.rs` shim, `RunError` → non-zero exit), selecting a `BundleSource` from CLI args.
**Depends on**: Phase 92 (toolkit module + `BundleSource`) and Phase 94 (stable `pmcp.toml` contract)
**Requirements**: WBCL-06
**Success Criteria** (what must be TRUE):

  1. Running `pmcp-workbook-server --bundle-dir <dir> --bundle-id <id>` (optionally `--http`) stands up a live MCP server whose five tools are served entirely from the compiled bundle — zero user Rust written
  2. The binary selects a `BundleSource` from CLI args, runs the boot integrity gate, and surfaces a load/integrity failure as a typed `RunError` → non-zero exit (matching `pmcp-sql-server`'s behavior)
  3. The published binary (slot 9a) links only `pmcp-server-toolkit[workbook]` + `pmcp-workbook-runtime` — the purity gate confirms no reader in its tree

**Plans**: 2 plans
Plans:
**Wave 1**

- [x] 95-01-PLAN.md — Re-skin the pmcp-sql-server crate as pmcp-workbook-server: lib (run/serve/run_serving + RunError incl. BundleIdMismatch), cli Args (--bundle-dir/--bundle-id/--http loopback), main shim, build_server seam (LocalDirSource + --bundle-id assert + try_with_workbook_bundle), workspace registration, runnable example over the synthetic golden bundle

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 95-02-PLAN.md — Test trio (assemble surface / ephemeral-port HTTP smoke / mcp-tester parity through the real binary path) + proptest fuzz of the --bundle-id fail-closed guard + purity-check assertion for the reader-free served cone + CLAUDE.md slot-9a publish-order wiring

### Phase 96: Shape B Scaffold + Dialect-Version Declaration + Generalization Validation

**Goal**: `cargo pmcp new --kind workbook-server` scaffolds a thin binary over `BundleSource` + the served-tool toolkit module (Shape B); workbooks declare the dialect version they target (forward-compatible evolution); and the generalization gates — a second, non-lighthouse workbook compiling and serving end-to-end, plus an Excel-quirk fixture corpus — prove the manifest is truly synth-driven with no per-workbook Rust and no privileged single output.
**Depends on**: Phase 95 (the scaffold targets the Shape A wiring)
**Requirements**: WBCL-05, WBDL-02, WBEX-01, WBEX-02
**Success Criteria** (what must be TRUE):

  1. A developer can run `cargo pmcp new --kind workbook-server` to scaffold a runnable crate (Cargo.toml + `main.rs` using `EmbeddedSource` + sample `pmcp.toml` + sample bundle) — a thin shell over the toolkit module, mirroring `--kind sql-server`
  2. A workbook declares the dialect version it targets, and the compiler validates that declaration — enabling forward-compatible dialect evolution
  3. **GENERALIZATION GATE (WBEX-01):** a second, non-lighthouse example workbook compiles and serves end-to-end through the SDK path, and its server's `get_manifest` / `tools/list` schema reflects ITS OWN inputs with zero shared Rust and no privileged single output — proving the manifest-driven §5 fix generalized
  4. ✅ An Excel-quirk fixture corpus (1900 leap-year, empty-cell coercion, error propagation, half-rounding boundaries) verifies reconcile determinism beyond the single golden case (WBEX-02, 96-05: 8 quirks across both layers — scalar_eval unit tests + penny-reconcile mini fixtures graded via within_tol)

**Plans**: 5 plans

- [x] 96-01-PLAN.md — WBDL-02 dialect-version declaration: pmcp_dialect_version reader + semver-compat (fail-closed) + dialect consts/spec drift guard + fuzz/property ✅ (commits 14047806, 45e4fa4f, 4702a329)
- [x] 96-02-PLAN.md — WBCL-05 Shape B scaffold: `cargo pmcp new --kind workbook-server` template + dispatch + purity-safe Cargo.toml + EMBEDDED publish-safe assets (include_dir!) + lib seam + drift-lock/bundle-bytes/version-drift golden tests + scaffold-build & packaging smokes + example ✅ (commits 91933535, 736a1266, cf670b6b)
- [x] 96-03-PLAN.md — WBEX critical-path spike: reusable #[cfg(test)] rust_xlsxwriter fixture author (Excel identity) + 1900-leap-year disposition spike
- [x] 96-04-PLAN.md — WBEX-01 generalization gate: synthetic loan/mortgage rate-tier second workbook compiles via the generic driver + serves its OWN get_manifest/tools/list schema (loan keys present, tax keys absent, DISJOINT) behind the same five generic tool names; reemit_loan 9-assertion served-schema proof (incl. production-refusal T-96-10) + the in_* input-naming convention (mirrors out_*) ✅ (commits 6b622e95, a7529369)
- [x] 96-05-PLAN.md — WBEX-02 Excel-quirk corpus: 8 quirks in BOTH layers — scalar_eval unit tests (excel_round source of truth; 1900-leap >59/+1 components per SPIKE, no DATE) + mini penny-reconcile fixtures graded by retrieving the recomputed value + cached oracle through within_tol (cannot pass on compile-success alone); 3 of 4 named quirks have a reconcile fixture (error propagation is the scalar_eval-only stand-in — runtime Div clamps NaN->0 / preflight short-circuit); production-refusal spot check + quirk->WBEX-02 traceability map ✅ (commits e3cce105, 7fa7458f)

## Progress — v2.3 Milestone

**Execution order:** Phase 91 (runtime + purity gate + dialect) → Phase 92 (BundleSource + served-tool module, freezes the bundle contract) → Phase 93 (compiler + §5 fixes + promote gate) → Phase 94 (CLI + `pmcp.toml`) → Phase 95 (Shape A binary) → Phase 96 (Shape B scaffold + dialect-version + generalization gates). Strictly sequential: each phase's output is the next phase's dependency (runtime ← compiler/toolkit; contract frozen before compiler re-cut; CLI/binary/scaffold over the now-stable runtime+compiler).

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 91. Workbook Runtime + Purity Gate + Dialect | 3/3 | Complete    | 2026-06-10 |
| 92. BundleSource + Served-Tool Toolkit Module | 7/7 | Complete    | 2026-06-11 |
| 93. Workbook Compiler + §5 Fixes + Promote Gate | 7/7 | Complete    | 2026-06-13 |
| 94. CLI Subcommands + `pmcp.toml` | 6/6 | Complete    | 2026-06-14 |
| 95. Shape A Binary `pmcp-workbook-server` | 2/2 | Complete    | 2026-06-14 |
| 96. Shape B Scaffold + Dialect-Version + Generalization | 5/5 | Complete    | 2026-06-15 |

## Phase Details — cargo-pmcp deploy (Phases 98-104, pre-SDK-extraction)

### Phase 98: `cargo pmcp deploy` — stack.ts Regeneration Guard + Config-Driven Metadata

**Goal**: `cargo pmcp deploy` stops silently overwriting an operator-curated `deploy/lib/stack.ts`, and curated template metadata (`mcp:serverType`, `mcp:snapshotBaked`) becomes reproducible-from-config so it survives any regeneration. Closes the defect diagnosed in `.planning/debug/deploy-overwrites-stack-ts.md`: both deploy targets do an unconditional `std::fs::write(stack.ts)` (no exists-guard, no diff, no opt-out), and `mcp:serverType`/`mcp:snapshotBaked` cannot be driven from `.pmcp/deploy.toml` (serverType hardcoded `'custom'` for custom/pmcp.toml servers; snapshotBaked has zero representation).

**Depends on**: none (standalone deploy-correctness fix; independent of Phase 97's GitHub-automation work, though it shares the `deploy.rs` / `.pmcp/deploy.toml` surface)
**Requirements**: DSTK-01, DSTK-02, DSTK-03, DSTK-04
**Success Criteria** (what must be TRUE):

  1. Running `cargo pmcp deploy` against a directory with a pre-existing, operator-edited `deploy/lib/stack.ts` leaves that file byte-for-byte unchanged on BOTH targets (pmcp-run + aws-lambda); IAM validation still runs and a "preserved existing stack.ts" notice is printed
  2. Passing `--regenerate-stack` (or `--force`) re-renders `stack.ts` from the template as before — the opt-out is explicit, not the default
  3. A `[metadata]` block in `.pmcp/deploy.toml` (`server_type = "graph-rag"`, `snapshot_baked = true`) flows through `render_stack_ts` / `McpMetadata` / `to_cdk_context` so the synthesized `stack.ts` advertises `mcp:serverType:'graph-rag'` + `mcp:snapshotBaked:'true'` — reproducible from config, surviving a regeneration
  4. ALWAYS coverage present and green: exists-guard unit tests on both targets, config-survives-render unit/property tests, golden-file update in `tests/backward_compat_stack_ts.rs` for the new `mcp:snapshotBaked` line, `--regenerate-stack` documented in `cargo-pmcp/docs/commands/deploy.md`; `make quality-gate` passes

**Source**: debug session `.planning/debug/deploy-overwrites-stack-ts.md` (root cause + recommended fix direction recorded under Resolution)

**Plans:** 4/4 plans complete

- [x] 98-01-PLAN.md — Config contract (`[metadata]` block + `regenerate_stack` runtime flag on DeployConfig) + RED regression tests reproducing the overwrite + config-metadata defects [DSTK-02] ✅ 2026-06-16
- [x] 98-02-PLAN.md — DSTK-01 exists-guard + `--regenerate-stack`/`--force` flag on BOTH targets (shared guarded-write helper, IAM validation preserved, "preserved existing stack.ts" notice) [DSTK-01]
- [x] 98-03-PLAN.md — DSTK-02 + DSTK-03 config-driven metadata (`McpMetadata.snapshot_baked` + `server_type` override → `to_cdk_context` `mcp:snapshotBaked` → template literal) [DSTK-02, DSTK-03]
- [x] 98-04-PLAN.md — DSTK-04 ALWAYS coverage (property test, `[metadata]` fuzz target, golden-file update, runnable example, `--regenerate-stack` + `[metadata]` docs) + `make quality-gate` green [DSTK-04]

### Phase 99: Workbook-Crate Cognitive-Complexity Reduction (PMAT gate debt)

**Goal**: Make `pmat quality-gate --fail-on-violation --checks complexity` pass workspace-wide by refactoring the 21 cognitive-complexity violations in the v2.3 workbook crates to the gate threshold, WITHOUT weakening the gate (`#[allow]` is a no-op for PMAT per Phase 75 D-10-B; no production crate goes into `.pmatignore`). Behavior is preserved by the milestone's existing golden/reconcile/quirk test net. Unblocks PR #279's complexity gate (the Makefile `SHELL := bash` fix already unblocked the separate purity-check gate).

**Depends on**: none (refactor of already-merged-on-branch milestone code; independent of Phase 98)
**Requirements**: CPLX-01, CPLX-02, CPLX-03, CPLX-04
**Success Criteria** (what must be TRUE):

  1. `pmat quality-gate --fail-on-violation --checks complexity` exits 0 with ZERO violations across the workspace
  2. All 21 flagged functions are refactored (the 5 over the cog-50 hard cap — `render_xlsx` 93, `classify_cell_roles` 74, `eval_expr` 58, `ingest` 57, `tokenize` 52 — by genuine decomposition); no production crate is added to `.pmatignore`; no `#[allow(clippy::cognitive_complexity)]` is relied on to clear the PMAT gate
  3. No behavior regressions — full workspace test suite green (golden/reconcile/quirk fixtures, dialect linter, provenance gate) and `make quality-gate` green
  4. PR #279's CI complexity gate goes green on the next run

**Source**: PR #279 CI failure (21 PMAT complexity violations); empirical PMAT-allow behavior in `.planning/phases/75-fix-pmat-issues/pmat-allow-behavior.md` (D-10-B)

**Plans**: 11 plans (10 parallel refactor plans + 1 gate-closure verification)

**Wave 1** *(disjoint files — fully parallel)*

- [x] 99-01-PLAN.md — render_xlsx (93) decomposition [runtime] [CPLX-01]
- [x] 99-02-PLAN.md — eval_expr (58) + f_index (24) + f_search (31) [runtime] [CPLX-01]
- [x] 99-03-PLAN.md — bundle_loader::load (28) [runtime] [CPLX-01]
- [x] 99-04-PLAN.md — classify_cell_roles (74) + dependency_order (24) [compiler] [CPLX-02]
- [x] 99-05-PLAN.md — tokenize (52) + lex_quoted_sheet_ref (33) + scan_atom_run (30) [compiler] [CPLX-02]
- [x] 99-06-PLAN.md — ingest (57) + references_external_workbook (31) [compiler] [CPLX-02]
- [x] 99-07-PLAN.md — parse_calc_pr (44) + parse_app_props (39) + gate_inner (29) [compiler] [CPLX-02]
- [x] 99-08-PLAN.md — derive_case_grid (34) + no_seeded_value_outside_allowed (46) [compiler] [CPLX-02]
- [x] 99-09-PLAN.md — extract_function_tokens (29) + author_xlsx (29) + walk (25) [compiler] [CPLX-02]
- [x] 99-10-PLAN.md — validate_input (33) [server-toolkit] [CPLX-03]

**Wave 2** *(blocked on all of Wave 1)*

- [x] 99-11-PLAN.md — gate-closure verification: `pmat quality-gate --checks complexity` zero violations + full workspace tests + `make quality-gate` green; assert no `.pmatignore`/`#[allow]` weakening [CPLX-01/02/03/04]

### Phase 104: Task-Augmented Tool Results DX (SEP-1686 junction)

**Goal**: Close the junction between the tool contract and the tasks layer so a tool can return a task-augmented (or otherwise full) `CallToolResult` — `_meta` included — through the normal `Server` dispatch front door, instead of dispatch stringifying it into `content[0].text`. Kills the silent double-wrap bug class documented by the pmcp.run team (5 incident variants, incl. a 2-week silent production outage), and lets their three hand-rolled pre-2.11 task servers migrate onto native TaskSupport.

**Depends on**: Phase 101 (tools-as-Tasks DX, pmcp 2.10.0), Phase 102 (HTTP task dispatch, pmcp 2.11.0)
**Requirements**: TOUT-01, TOUT-02, TOUT-03, TOUT-04
**Success Criteria** (what must be TRUE):

  1. A `ToolHandler` has a typed, explicit way to return a full `CallToolResult` (with `_meta`) that lands on the wire un-re-wrapped through normal `Server` dispatch (TOUT-01) — implicit "parses as CallToolResult" sniffing is explicitly rejected (fully-defaulted serde makes any JSON object parse)
  2. Dispatch emits a WARN (debug-fail optional) when about to text-wrap a `Value` that structurally looks like an already-built `CallToolResult` (high-precision markers: valid `content` array of Content items, or `_meta` containing `RELATED_TASK_META_KEY`) (TOUT-02)
  3. The client exposes a typed `related_task()` accessor on tool-call results (SEP-1686 `_meta["io.modelcontextprotocol/related-task"]` → `TaskMetadata`) so integrators stop hand-rolling task detection (TOUT-03)
  4. A migration guide documents hand-rolled `_meta` task patterns → native 2.11 `with_task_store()` machinery, incl. the Required-without-store build validation and confirmation that native `CreateTaskResult` `_meta` emission (D-08/D-09) is compatible with `_meta`-sniffing clients (TOUT-04)
  5. ALWAYS requirements met (unit + property + fuzz + runnable example) and `make quality-gate` green

**Source**: pmcp.run team issue `pmcp-run/.planning/notes/sdk-issue-tool-as-task-dx.md` (2026-06-21 + 2026-07-04 addendum); verified against `src/server/mod.rs:1493` (unconditional text-wrap) and `src/server/core_tests.rs:881` (native `_meta` emission)

**Plans:** 5/5 plans complete

Plans:

**Wave 1**

- [x] 104-01-PLAN.md — TaskMetadata + CallToolResult::{with_related_task, related_task} + Client::wait_for_task (wasm-safe, runtime::sleep) [TOUT-03, TOUT-01/D-03.1]
- [x] 104-02-PLAN.md — ToolOutput enum + ToolHandler::handle_output default + shared verbatim pass-through in both dispatchers (D-04/D-05) [TOUT-01]

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 104-03-PLAN.md — Double-wrap tripwire (looks_like_call_tool_result WARN+debug_assert) + per-tool suppress_double_wrap_check [TOUT-02]

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 104-04-PLAN.md — ServerBuilder::tool_with_result + RequestHandlerExtra::set_result_meta (interior-mut slot) [TOUT-01/D-03.2/D-03.3]

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 104-05-PLAN.md — s47 BEFORE/AFTER example + live-HTTP _meta-at-top-level acceptance gate + migration guide (docs/design + pmcp-book + README) [TOUT-04]

---

## v2.4 Agents & Teams — SDK Extraction (Phases 106-111)

**Milestone Goal:** Make the PMCP SDK the reference implementation for agents-as-MCP-clients and agent teams — a spec-compliant client host surface, portable contracts (`pmcp-package`), the `pmcp-agent` loop crate, dev-grade team reference servers, and cargo-pmcp verbs — per the approved design `docs/design/agents-teams-sdk-extraction-plan.md`. Boundary razor: contracts + reference implementations live in the open SDK; operation + scale stay on pmcp.run.

**Dependency spine (design §4):** compliance (HOST) → contracts (PKG) → agent (AGNT) → teams (TEAM) → CLI → docs. Phases 106 and 107 are independent and may run in parallel; 108 needs 106+107; 109 needs 108 (+107 fixtures); 110 needs 107-109; 111 documents shipped code from 106-110.

**Publish-order impact (design §5):** new entries `pmcp-package` (leaf, before cargo-pmcp), `pmcp-agent` (after `pmcp`), `pmcp-team-servers` (after `pmcp-agent`); cargo-pmcp moves after all three. All new crates are 0.x/experimental; `pmcp` core changes (Phase 106) are additive minor bumps.

- [x] **Phase 106: Client Host Surface** — a pmcp `Client` can host server→client sampling/elicitation/roots with a human-in-the-loop hook; legacy inverted sampling documented as the "LLM-server pattern" (design Phase A) (completed 2026-07-17)
- [x] **Phase 107: Contracts & Package Format** — `pmcp-package` adopted into this repo + published 0.1.0 (wire-frozen), team-server tool contracts captured as provable-contracts YAML (design Phase B) (completed 2026-07-18)
- [x] **Phase 108: `pmcp-agent` Loop Crate** — the pure agent loop between effect seams, three CompletionSources, agent-as-server adapter, tasks-aware ToolInvoker, configured from an AgentPackage (design Phase C) (completed 2026-07-18)
- [x] **Phase 109: Team Reference Servers** — `pmcp-team-servers` (one feature-flagged crate) with dev-grade team-fs/approval-mcp/mem-mcp/team-mcp + conformance tests against the PKG-03 contracts (design Phase D) (completed 2026-07-19)
- [x] **Phase 110: cargo-pmcp Agent & Team Verbs** — `agent new`/`agent dev`, `team dev`, `package capture|show` with version-pin tripwires (design Phase E) (completed 2026-07-19)
- [ ] **Phase 111: Docs in Three Shapes + Examples** — pmcp-book chapters, runnable examples, README + course updates leading with the cargo pmcp workflow (design Phase F)

## Phase Details — v2.4 (Agents & Teams)

### Phase 106: Client Host Surface

**Goal**: A pmcp `Client` can answer server→client requests (spec-direction `sampling/createMessage` incl. tools/tool_choice, `elicitation/create`, `roots/list`) through a client-side handler registry with a human-in-the-loop approval hook, and the legacy inverted sampling path is documented as the distinct "LLM-server pattern" — all additive (pmcp minor bump). Small, independently shippable, and unblocks Phase 108's `SamplingSource`.
**Depends on**: Nothing (first phase of milestone; parallelizable with Phase 107)
**Requirements**: HOST-01, HOST-02, HOST-03, HOST-04, HOST-05, HOST-06
**Success Criteria** (what must be TRUE):

  1. A developer registers a client-side `SamplingHandler` and a sampling-requesting server's `sampling/createMessage` (tools/tool_choice included) is answered instead of erroring "Unexpected message type" — proven by a duplex round-trip harness test (HOST-01)
  2. A developer registers an `ElicitationHandler` and a roots provider, and the client answers `elicitation/create` and `roots/list` (HOST-02, HOST-03)
  3. The sampling path invokes an async human-in-the-loop approval callback (default allow) before returning a completion, per the spec SHOULD (HOST-04)
  4. `ClientCapabilities` advertised on initialize reflect which host handlers are registered — sampling/elicitation/roots (HOST-05)
  5. The legacy `Client::create_message` → server `SamplingHandler` path is documented as the "LLM-server pattern", disambiguated from spec sampling in rustdoc and book, with zero breaking changes (HOST-06)

**Plans**: 3 plans

- [x] 106-01-PLAN.md — client::host module (traits, registry, preflight/result-review approval types, Result-returning roots provider) + roots wire-type relocation for wasm-clean surface + Client dispatch (classify_host_request, sanitized -32603) across all ctors/Clone + duplex round-trips (elicitation via raw pump) + registered s49 example + create_message LLM-server rustdoc (HOST-01/02/03 + HOST-06 rustdoc)
- [x] 106-02-PLAN.md — preflight approval gate (deny before the LLM runs) + optional result-review + registry-authoritative capabilities preserving caller sub-cap detail + parse_request->classify routing fuzz + pmcp 2.16.0 bump with cargo-pmcp pin tripwire (HOST-04, HOST-05)
- [x] 106-03-PLAN.md — pmcp-book Sampling & Hosting disambiguation page (real pmcp::SamplingHandler paths, preflight gate described) + SUMMARY link (HOST-06 book)

### Phase 107: Contracts & Package Format

**Goal**: The portability contracts exist, versioned and wire-frozen, with this repo as the canonical home — `pmcp-package` adopted (from `~/Development/mcp/sdk/pmcp-run/crates/pmcp-package`) and published 0.1.0, plus the four team servers' tool surfaces captured as provable-contracts YAML with shared conformance fixtures.
**Depends on**: Nothing (parallelizable with Phase 106; contract-first, precedes Phase 108/109 implementations per house rule)
**Requirements**: PKG-01, PKG-02, PKG-03
**Success Criteria** (what must be TRUE):

  1. `pmcp-package` builds in this repo as a standalone workspace-excluded crate with publish-ready metadata — public-facing description, README, license files, and docs.rs-verified rustdoc (PKG-01)
  2. A developer can depend on `pmcp-package = "0.1"` from crates.io; the wire-freeze policy (0.1.x = digest/serialization-stable, serialized-shape changes bump 0.2.0) is documented and enforced by passing golden fixtures (PKG-02)
  3. The team-server tool contracts — `fs__*`, `mem__*`, `team_mcp__<member>` dispatch, `resolve_approval`/`get_approval` + dynamic `team_approval__ask_*` — are captured as versioned provable-contracts YAML with shared conformance fixtures, marked as namespaced provisional PMCP extensions (PKG-03)

**Plans**: 3 plans

Plans:
**Wave 1**

- [x] 107-01-PLAN.md — Adopt pmcp-package into the repo with publish-ready metadata, license files, README (wire-freeze policy), and docs.rs-clean scrubbed rustdoc (PKG-01)
- [x] 107-03-PLAN.md — Author team-servers-v1.yaml (4 tool-surface equations) + shared conformance fixtures + structural conformance test (PKG-03)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 107-02-PLAN.md — Extend golden fixtures to all four package kinds + wire pmcp-package into publish order and release.yml (PKG-02)

### Phase 108: `pmcp-agent` Loop Crate

**Goal**: The agent runtime ships as an open, deploy-anywhere `crates/pmcp-agent` (0.x, experimental, isolated from `pmcp` core) — a pure decision loop between object-safe effect seams, three CompletionSources (sampling-first), an agent-as-server adapter, and a tasks-aware ToolInvoker, all configured from an `AgentPackage`. pmcp.run's `handler/iteration.rs` becomes a platform-specific composition of this loop.
**Depends on**: Phase 106 (`SamplingSource` uses the client host surface), Phase 107 (`AgentPackage` from `pmcp-package`)
**Requirements**: AGNT-01, AGNT-02, AGNT-03, AGNT-04, AGNT-05, AGNT-06, AGNT-07, AGNT-08, AGNT-09
**Success Criteria** (what must be TRUE):

  1. A developer implements against object-safe async `CompletionSource`/`ToolInvoker`/`ConversationStore` seams, with `CompletionSource` reusing the SDK sampling types verbatim (AGNT-01)
  2. The replay-safety invariant is property-tested over recorded effect traces: identical effect results ⇒ identical loop decisions, and the iteration loop runs pure between seams with retry classification exposed as data (no retry/backoff policy inside the loop) (AGNT-02, AGNT-03)
  3. The same loop runs against `SamplingSource` (zero-dep spec sampling incl. tools/tool_choice), feature-gated `OpenAiCompatSource`, and feature-gated `AnthropicSource` — proven by the standalone-vs-sampled example (AGNT-04, AGNT-05, AGNT-06)
  4. An agent is exposed as an MCP server via a high-level `pmcp::Server` adapter (native-only, deployable through the existing Lambda/Docker target adapters that host `pmcp::Server::run<T: Transport>`; per D-13 the wasm32 CI gate proves the loop + seams + config path is target-clean, and the adapter/SamplingSource are native-only because they ride pmcp's native-only `task_store`/`PeerHandle` — per-target deploy demos are Phase 110/111 scope), and its `ToolInvoker` over `pmcp::Client` honors task-augmented tool results via `poll_decision` (SEP-1686) (AGNT-07, AGNT-08)
  5. An agent is fully configured from an `AgentPackage` plus resolved config slots — the same definition drives laptop, deploy targets, and platform (AGNT-09)

**Plans**: 6 plans in 4 waves

- [x] 108-01-PLAN.md — pmcp 2.17.0 core: D-106-A response pump + `sample_with_tools` peer path (wave 1)
- [x] 108-02-PLAN.md — `pmcp-agent` crate scaffold + three object-safe effect seams (AGNT-01) (wave 2)
- [x] 108-03-PLAN.md — pure decision core + iteration engine + EffectTrace replay-safety + fuzz (AGNT-02, AGNT-03) (wave 3)
- [x] 108-04-PLAN.md — three CompletionSources: SamplingSource + OpenAiCompatSource + AnthropicSource (AGNT-04/05/06) (wave 3)
- [x] 108-05-PLAN.md — tasks-aware ClientToolInvoker + SlotResolver/endpoint config (AGNT-08, AGNT-09) (wave 3)
- [x] 108-06-PLAN.md — agent-as-server adapter + standalone-vs-sampled example + wasm32 gate + D-09 mapping (AGNT-07, AGNT-04/05/06 proof) (wave 4)

### Phase 109: Team Reference Servers

**Goal**: The four team servers exist as open reference implementations with dev-grade backends in one feature-flagged crate `crates/pmcp-team-servers`; "small team, one process" works locally, and conformance tests prove each server's tool surface matches the Phase 107 (PKG-03) contract fixtures — the same fixtures the platform servers can run.
**Depends on**: Phase 108 (team-mcp composes agent-as-server members), Phase 107 (contract fixtures)
**Requirements**: TEAM-01, TEAM-02, TEAM-03, TEAM-04, TEAM-05, TEAM-06
**Success Criteria** (what must be TRUE):

  1. `crates/pmcp-team-servers` builds with per-server feature flags and runnable dev binaries for all four servers (TEAM-01)
  2. team-fs serves `fs__*` over a `TeamFsBackend` trait with a local-directory dev backend, and mem-mcp serves `mem__*` over a `TeamMemoryBackend` trait with a keyword/BM25 in-memory dev backend (no embedder dependency) (TEAM-02, TEAM-04)
  3. approval-mcp serves the approval contract over an in-memory `TaskStore` with console (dev) and webhook (CI) approval channels (TEAM-03)
  4. team-mcp composes Phase 108 agent-as-server members as per-member tools returning `ToolOutput::Result` with top-level `related_task` `_meta` — the worked migration template replacing the platform's raw-JSON-RPC bypass (TEAM-05)
  5. Conformance tests prove each reference server's tool surface matches the PKG-03 contracts (TEAM-06)

**Plans**: 9 plans in 4 waves (replanned 2026-07-18 from cross-AI review — added prerequisite pmcp-core `_meta` enablement plan 109-00)

Wave 1 (parallel):

- [x] 109-00-PLAN.md — pmcp-core `_meta` enablement (prerequisite): extensible `RequestMeta` + `RequestHandlerExtra.request_meta` + `call_tool_with_task_and_meta`/`call_tool_with_meta` (unblocks D-14 guard `_meta`; 109-05/07 depend on it)
- [x] 109-01-PLAN.md — Crate scaffold + documented module skeleton + exported transport + PackageResolver/MemberId seams + `derive_attachment` (atomic, D-05/D-07) + additive contract rev to v1.1.0 with CORRECT related-task key + contract-first binding.yaml skeleton (D-12/D-14/D-18)

Wave 2 (parallel):

- [x] 109-02-PLAN.md — team-fs: `TeamFsBackend` + `LocalDirBackend` (pure lexical containment, symlink reject, percent-encoded file://, review/ sync) + HTTP-first binary (TEAM-02)
- [x] 109-03-PLAN.md — mem-mcp: `TeamMemoryBackend` + numerically-safe zero-dep BM25 (L_avg==0 short-circuit, IDF floor) + deterministic ids + HTTP-first binary (TEAM-04)
- [x] 109-04-PLAN.md — approval-mcp: InMemoryTaskStore + ApprovalRepository (service-owner, atomic option-validated resolve) + bounded-timeout notify-only channels + subject-ref linkage + HTTP-first binary (TEAM-03)
- [x] 109-05-PLAN.md — team-mcp: task+`_meta`-forwarding member hop (explicit Task/Result forwarding contract) → related-task under `RELATED_TASK_META_KEY`, MemberId guards, PackageResolver, injected-override LLM, fuzz, HTTP-first binary (TEAM-05; depends on 109-00)

Wave 3 (parallel):

- [x] 109-06-PLAN.md — In-process `TeamRuntimeBuilder`/`TeamRuntime` (all seams, cfg-gated + fail-closed attachment, transactional startup) + "small team, one process" tests (D-01/D-04/D-06/D-15, TEAM-01)
- [x] 109-07-PLAN.md — Exportable wire-level conformance runner over a `ConformanceTarget` (in-mem+HTTP) + fixture schema v2 (deterministic ids, scenarios, expected schemas) + every-tool/every-guard fixtures + negative harness (TEAM-06, D-17/D-19/D-20; depends on 109-00)

Wave 4:

- [x] 109-08-PLAN.md — finalize binding.yaml + correct `pmat comply check --path .` (fail-closed comply-ci in CI, graceful comply locally) + doc-review E2E example + all-four subprocess smoke via SDK stdio client (D-16/D-18)

### Phase 110: cargo-pmcp Agent & Team Verbs

**Goal**: cargo-pmcp is the on-ramp for agents and teams, matching its server story — `agent new`/`agent dev`, `team dev` (in-process small team from a `TeamPackage`), and `package capture|show` (thin clients to the platform capture API), each with version-pin tripwires. Agents deploy through the existing target adapters (an agent-as-server is just a server binary; AgentCore is a deferred follow-on adapter).
**Depends on**: Phase 107 (`package capture` uses `pmcp-package`), Phase 108 (`agent new`/`agent dev`), Phase 109 (`team dev` wires the four reference servers)
**Requirements**: CLI-01, CLI-02, CLI-03, CLI-04
**Success Criteria** (what must be TRUE):

  1. `cargo pmcp agent new` scaffolds an agent project (AgentPackage manifest + standalone runner) with a version-pin tripwire test against `pmcp-agent` (CLI-01)
  2. `cargo pmcp agent dev` runs an agent locally against an OpenAI-compat endpoint or as a sampling-hosted server (CLI-02)
  3. `cargo pmcp team dev` runs an in-process small team — member agents + all four reference team servers with dev backends — wired from a `TeamPackage` (CLI-03)
  4. `cargo pmcp package capture|show` work as thin clients to the platform capture API with `pmcp-package = "0.1"` (caret) and a pin tripwire test against version drift (CLI-04)

**Plans**: 6 plans

Plans:
**Wave 1**

- [x] 110-01-PLAN.md — Foundation: dependency wiring + agent/team/package command groups + main.rs arms (wave 1)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 110-02-PLAN.md — `agent new` scaffolder + AgentPackage manifest + pmcp-agent pin tripwire (CLI-01, wave 2)
- [x] 110-03-PLAN.md — `agent dev` --source openai-compat|sampling|fixed + in-process sampling test (CLI-02, wave 2)
- [x] 110-04-PLAN.md — `team dev` offline doc-review transcript + --serve/--llm (CLI-03, wave 2)
- [x] 110-05-PLAN.md — `package show|capture` + pure kind::detect_kind + pmcp-package caret pin tripwire (CLI-04, wave 2)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 110-06-PLAN.md — ALWAYS deliverables: agent + team-dev examples (over production seams) + manifest-parse fuzz + #[doc(hidden)] lib seams (CLI-01/02/03/04, wave 3)

### Phase 111: Docs in Three Shapes + Examples

**Goal**: The milestone is documented in three shapes per the house rule (README + pmcp-book chapter + pmcp-course chapter), leading with the `cargo pmcp` workflow and the deploy-anywhere/preferred-pmcp.run positioning, with runnable examples verified against the shipped Phase 106-110 code.
**Depends on**: Phases 106-110 (documents shipped code)
**Requirements**: DOCS-01, DOCS-02, DOCS-03
**Success Criteria** (what must be TRUE):

  1. pmcp-book ships the "Agents as MCP Clients", "Agent Teams", and "Sampling & Hosting" chapters (incl. the LLM-server pattern disambiguation) (DOCS-01)
  2. Runnable examples ship and pass: sampling host, standalone-vs-hosted agent (same loop, two sources), and small team end-to-end (DOCS-02)
  3. README + pmcp-course are updated per the three-shapes rule, leading with the `cargo pmcp` workflow and the deploy-anywhere/preferred-pmcp.run positioning (DOCS-03)

**Plans**: TBD

## Progress — v2.4 Milestone (Agents & Teams — SDK Extraction)

**Execution order:** Phases 106 and 107 in parallel → Phase 108 (needs 106+107) → Phase 109 (needs 108, +107 fixtures) → Phase 110 (needs 107-109) → Phase 111 (documents 106-110). Contract-first: Phase 107 contracts precede the Phase 108/109 implementations.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 106. Client Host Surface | 3/3 | Complete    | 2026-07-18 |
| 107. Contracts & Package Format | 3/3 | Complete    | 2026-07-18 |
| 108. `pmcp-agent` Loop Crate | 6/6 | Complete   | 2026-07-18 |
| 109. Team Reference Servers | 9/9 | Complete    | 2026-07-19 |
| 110. cargo-pmcp Agent & Team Verbs | 6/6 | Complete    | 2026-07-19 |
| 111. Docs in Three Shapes + Examples | 0/TBD | Not started | - |

## v2.5 MCP Spec 2026-07-28 (v2) Support (Phases 112-119) — ✅ SHIPPED 2026-08-22

Published as **pmcp v2.19.0** (PR #337, tag v2.19.0). 11 phases, 176 plans, 337 tasks — all
verification `passed`. One binary now serves both MCP 2025-11-25 and 2026-07-28 via per-request
negotiation; v2 primary (stateless, Tasks-as-extension, JSON Schema 2020-12), v1 cleanly severable
at compile time. Wholly additive — a 2.x minor.

<details>
<summary>Phases 112-119 (all complete)</summary>

- [x] Phase 112: Version plumbing spine (10/10 plans) — VERS-01..09
- [x] Phase 113: Stateless HTTP + multi-round-trip elicitation (32/32 plans) — HTTP-01..08
- [x] Phase 113.1: Merge unblock (6/6 plans)
- [x] Phase 114: Tasks extension migration (20/20 plans) — TASK-01..06
- [x] Phase 115: JSON Schema 2020-12, structured output, caching hints (20/20 plans) — SCHM-*
- [x] Phase 116: Auth-hardening SEPs (16/16 plans) — AUTH-*
- [x] Phase 117: Agents, tester & v1 severability (14/14 plans) — SMPL-01/02, CLNT-03/04
- [x] Phase 118: Conformance against the official suite (10/10 plans) — CONF-*
- [x] Phase 118.1: Close the nine conformance gaps G-1..G-9 (14/14 plans)
- [x] Phase 118.2: v1 client SSE transport + `notifications/message` emitter (24/24 plans)
- [x] Phase 119: Documentation in three shapes, v2 migration (10/10 plans) — DOCS-*

</details>

**Full phase detail, requirements and progress tables:** `.planning/milestones/v2.5-ROADMAP.md`
and `.planning/milestones/v2.5-REQUIREMENTS.md`. Milestone record: `.planning/MILESTONES.md`.

---

## v2.6 AI-Package Portability (Phases 120-124)

**Milestone Goal:** Make an AI-Package genuinely portable between pmcp.run environments — build a
server from **configuration only**, test and attest it in one AWS account/region, export it, and
import it into another with the target environment told exactly what it must supply. The proving
case is `pmcp-openapi-server`: a Shape A pure-config binary whose entire identity is a `config.toml`
plus an OpenAPI spec.

**Why now, and why shaped this way.** `pmcp-package` 0.1.0 already has the primitives — local OCI
layout, `pack_server`/`unpack_server`, canonical digest + `verify`, and the config-slot machinery
(`classify` / `aggregate` / `detect_deviation`). What it lacks is the ability to express a server
that has *no bespoke binary*, any transport off the local disk, and any notion of attestation.
`cargo pmcp package` today has exactly one verb: `inspect`.

*Re-measured at milestone open (2026-08-22): that last sentence is now stale on `main` —*
*`cargo-pmcp/src/commands/package/mod.rs` enumerates five verbs (`inspect | capture | show |`*
*`import | approve`), and `import` is already taken by the remote workflow-manifest dry-run*
*import. The gap this milestone closes is unchanged: none of the five packs or unpacks an*
*AI-Package. See the Phase 123 reality check.*

**Two decisions taken at milestone scoping (2026-07-27), both of which SHRINK the SDK's share:**

1. **Attestation is pmcp.run-issued.** The GraphQL endpoint issues/attests a signature when a
   version is promoted; trust is anchored in pmcp.run, not in a developer-held key. The SDK's job
   is therefore *carriage and verification*, NOT signing — **no crypto dependency is added to
   `pmcp-package`.** (`digest::verify` is and remains an integrity check, not a signature check.)

2. **GraphQL mediates import.** The package is uploaded through pmcp.run's endpoint, which owns
   placement into ECR. **`oci-client` is therefore NOT added** — the CLI never speaks to a registry.
   `oci-spec` (types only) stays; the manifest types were already chosen so a registry client
   consumes them with zero translation, which keeps that door open.

**The consequence, stated plainly:** both decisions put the critical path in the pmcp.run backend,
outside this repo. Phases 122-123 are therefore **contract-first and parked** — they land a vendored
GraphQL contract plus an offline blocking contract test, exactly the pattern
`feat/package-remote-capture-show` already used for `capture-v1.graphql`, and go green when the
backend ships. Phases 120-121 depend on nothing external and are where the durable value is.

**Branch:** this milestone continues on a rebased `feat/package-remote-capture-show` — **249 commits
ahead of `main`, 20 behind** (re-measured 2026-08-22), **zero overlap** with `src/server/`,
`src/shared/`, `src/types/`, so it does not collide with v2.5. That branch already gated its own
release tag on an import E2E; this milestone is finishing what it deliberately left open, not
starting fresh.

**Non-goals:** signing keys or PKI in the SDK (decision 1); an ECR client in the CLI (decision 2);
changing `LATEST_PROTOCOL_VERSION` (that is a v2.5 concern and stays pinned); refactoring the
manifest schema for elegance — the schema is expected to churn, so the E2E is the asset, not the API.

Byte-for-byte round-tripping of the package is also a non-goal — tool-list parity is the property
that matters; byte identity would break on every manifest revision without indicating a real
regression.

**Decisions taken at milestone open (2026-08-22)** — settled, not open:

1. **Scope taken as scoped.** Phases 120-124, their names and their goals are fixed as written at
   the 2026-07-27 scoping pass. No re-derivation, no renumbering, no added phase.

2. **Phases 122 and 123 stay PARKED / contract-first.** A vendored contract plus an offline blocking
   contract test only — **no live E2E leg this milestone**. Every success criterion for both phases
   is achievable entirely offline, inside this repo, with the pmcp.run backend unavailable.

3. **UNAS-01 (SEP-2243 `x-mcp-header` / `Mcp-Param-{Name}`) is deferred again** and gets no phase —
   there is no Phase 125. It stays in Future Requirements, unassigned, with its measurement attached.

4. **Work continues on a rebased `feat/package-remote-capture-show`**, not a fresh branch off `main`.

- [x] **Phase 120: Config-Server Packaging** — `pack_server` currently demands `bootstrap: &[u8]`, so a config-only server cannot be expressed. Add vendor media types for the server's own `config.toml` and its OpenAPI spec as layers, and make the binary **dual-mode**: embedded (bootstrap bytes, for a new server or a new version) or referenced (`BinaryRef { digest, media_type }` resolved in the target environment, for a server already deployed there). Both modes are required. Decide and document what is *baked* versus what is a *slot* — the working split is that the spec is baked (it defines the tool surface; change it and it is a different package) while endpoint, credentials and auth mode are slots. (completed 2026-08-23; checkbox ticked 2026-08-25 — the Progress table and `120-VERIFICATION.md` (`status: passed`) had recorded completion since 2026-08-23, but this checkbox was left `[ ]`, which made `phase.complete` route back to an already-finished phase twice)
- [x] **Phase 121: Local Round-Trip E2E** — the regression net, and the piece that needs no backend. Using the London Tube fixture already in `crates/pmcp-openapi-server/tests/fixtures/`: pack in env A → unpack in env B → `required_slots` names **exactly** the slots B must fill (and `detect_deviation` separately reports B's endpoint drift) → fill them → assert **tool-list parity** with A via the existing `parity_replay.rs`. Parity is the property; byte round-tripping is not. This test must survive an arbitrary number of manifest-shape refactors, so assert on behaviour, not on manifest structure. (completed 2026-08-25)
- [x] **Phase 122: Attestation Carriage** *(contract-first, parked on backend)* — a layer to hold a pmcp.run-issued attestation and a verification path against pmcp.run's identity. No signing, no crypto dependency. Vendor the attestation contract from the live platform and write the offline blocking contract test; the live half activates when the backend issues attestations. (completed 2026-08-25)
- [x] **Phase 123: Export/Import Verbs** *(contract-first, parked on backend)* — `cargo pmcp package save | load | pull`, resolving environments through `configure`'s existing resolver and reusing the working `deployment/targets/pmcp_run/{graphql,auth}.rs` seam (`PMCP_API_URL`, token cache + TTL) rather than inventing a second API path. `save`/`load` are the local file round trip and can land immediately; `pull` is contract-first against `getPackageArtifact`. *(Verb set restated 2026-08-26 — `pack`/`unpack` became `save`/`load`, `export` is retired, and `import` stays the platform's, unchanged. See the Phase 123 detail section and `123-CONTEXT.md` D-01/D-02/D-03.)* (completed 2026-08-26)
- [ ] **Phase 124: Release & Publish Order** — `pmcp-openapi-server` is **absent from CLAUDE.md's publish order** (zero occurrences) and would silently not publish, unlike its siblings `pmcp-sql-server` and `pmcp-workbook-server`. Add it, publish `pmcp-package` 0.2.0 and `cargo-pmcp` 0.19.0, and record the ordering constraint that `pmcp-package` precedes `pmcp-agent` and `cargo-pmcp`. *(Re-measured 2026-08-22: the absence was closed on `main` — slot 9b in CLAUDE.md plus a `release.yml` step and the `release-coverage` CI gate. Two residual gaps remain; see the Phase 124 reality check.)*

## Phase Details — Current Milestone

*Milestone **v2.6 AI-Package Portability** — Phases 120-124, opened 2026-08-22. Requirement text and
the authoritative traceability table live in `.planning/REQUIREMENTS.md`. Phases 120-124 were scoped
on 2026-07-27 at the v2.5 close and taken as scoped at the open; the success criteria below are the
piece that scoping did not produce.*

### Phase 120: Config-Server Packaging

**Goal**: A server whose entire identity is a `config.toml` plus an OpenAPI spec has a complete package identity — vendor media types carry both as layers, the binary is dual-mode (embedded bootstrap bytes, or a `BinaryRef { digest, media_type }` resolved in the target environment), and the baked-versus-slot split is decided, documented and machine-checkable. Today `pack_server` (`crates/pmcp-package/src/oci/pack.rs:51`) takes `bootstrap: &[u8]` as a required positional parameter, so a config-only server is literally unrepresentable.
**Depends on**: Nothing (first phase of the milestone; runs together with Phase 121)
**Requirements**: PKG-01, PKG-02, PKG-03
**Success Criteria** (what must be TRUE):

  1. A `pmcp-openapi-server` package built from `crates/pmcp-openapi-server/tests/fixtures/london-tube.toml` + `london-tube-api.yaml` packs to a local OCI layout with **no bootstrap layer at all** — the manifest carries a `config.toml` layer and an OpenAPI-spec layer under new `application/vnd.pmcp.*` vendor media types (siblings of `MT_SERVER_BOOTSTRAP` in `crates/pmcp-package/src/oci/media_types.rs`), and `unpack_server` restores both files byte-identically (PKG-01)
  2. Both binary modes round-trip: an **embedded** package carries bootstrap bytes as today, and a **referenced** package carries only `BinaryRef { digest, media_type }` (`crates/pmcp-package/src/package/server.rs:361`) with no bootstrap blob in the layout. Unpacking a referenced package in an environment that does not hold the blob reports the digest that must be resolved rather than failing with a missing-layer error, and a caller cannot mistake a referenced package for one that has bytes (PKG-02)
  3. The baked-versus-slot split is enforced, not merely written down: changing one byte of `london-tube-api.yaml` changes the package's canonical manifest digest and `digest::verify` (`crates/pmcp-package/src/digest/verify.rs:28`) rejects the stale digest — while endpoint, credentials and auth mode surface as `ConfigSlot`s that `classify` (`crates/pmcp-package/src/slot/classification.rs:24`) sorts and `aggregate` (`crates/pmcp-package/src/slot/aggregate.rs:23`) returns, with no spec-derived slot among them (PKG-03)
  4. A golden fixture pins the config-only package kind's canonical digest under `crates/pmcp-package/tests/golden_fixtures/`, so a later change to the layer set, layer order or media-type strings fails `crates/pmcp-package/tests/digest_stability.rs` instead of silently shipping a package the previously published CLI cannot read (PKG-01, PKG-02)

**Plans**: 5/5 plans executed (4 waves)

Plans:
**Wave 1**

- [x] 120-01-PLAN.md — Tracer: config-only pack/unpack end-to-end, plus the one-way 0.2.0 wire break (wave 1)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 120-02-PLAN.md — Optional OpenAPI spec layer, 0.1.x refusal, media-type index hardening (wave 2)
- [x] 120-04-PLAN.md — Toolkit `[[config_slots]]` + `base_url` `${VAR}` expansion + the london-tube proving fixture (wave 2, parallel with 120-02)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 120-03-PLAN.md — Slot vocabulary: `SlotType::Endpoint`/`AuthMode`, `ConfigSlot.config_key` plus its repo-wide 43-literal source migration, `required_slots` (wave 3)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 120-05-PLAN.md — `[[config_slots]]` declaration agreement (D-01 through the real API path), pack-time placeholder validation, the packed-manifest golden (wave 4)

### Phase 121: Local Round-Trip E2E

**Goal**: The regression net — and the piece that needs no backend. A package moves between two environments and the property asserted is **tool-list parity**: pack in A, unpack in B, `required_slots` names exactly the slots B must fill (with `detect_deviation` separately reporting B's endpoint drift), fill them, and B serves the same tools as A. It is written to survive an arbitrary number of manifest-shape refactors, because the E2E is the asset this milestone leaves behind, not the manifest API.
**Depends on**: Phase 120 (needs the config-only package shape and the referenced-binary mode)
**Requirements**: PKG-04
**Success Criteria** (what must be TRUE):

  1. A round-trip test packs the london-tube server in a simulated environment A and unpacks it in a distinct environment B — separate OCI layouts, separate temp dirs, different endpoint/credential/auth-mode values, no shared process state — and runs fully offline against a `wiremock` backend with no live network, exactly as `crates/pmcp-openapi-server/tests/parity_replay.rs` already does (PKG-04)
  2. In environment B, `required_slots` (`crates/pmcp-package/src/slot/required.rs:85`) names **exactly** the slots B must fill — asserted as set equality against an explicit hardcoded expected list, so a slot added later that B is never told about turns the test red rather than being silently defaulted. Separately, once B fills its endpoint with a value differing from A's tested value, `detect_deviation` (`crates/pmcp-package/src/slot/deviation.rs:28`) must report that drift (PKG-04)

     > **Corrected 2026-08-23 (Phase 121 discussion, D-04/D-05).** This criterion previously routed the set-equality assertion through `detect_deviation`, which structurally cannot satisfy it: that function compares one already-known `(tested, proposed)` pair and short-circuits on identity-bearing slots, so it can never name the `TFL_APP_KEY` credential. The london-tube fixture has three slots (endpoint + auth_mode behavior-relevant, secret identity-bearing), so the original wording would have asserted a 2-slot set where the truth is 3 — silently omitting the credential, the most important thing environment B must supply. `required_slots` is the enumerator; its own doctest already states *"The credential IS enumerated here — `detect_deviation` could never name it."* Both functions are exercised, each for what it actually does.

  3. Once those named slots are filled, the environment-B binary serves a tool list set-equal to environment A's, and `london-tube-scenarios.yaml` replays green through `mcp-tester`'s `ScenarioExecutor` with per-step gating — the same harness `parity_replay.rs` uses, so parity is asserted on served behaviour (PKG-04)
  4. The test is proven insensitive to manifest shape and sensitive to real regressions, both directions exercised: adding a field to `ServerPackage` leaves it green, while dropping a tool from B's served surface or leaving a named slot unfilled turns it red. It contains no assertion on manifest field names, layer ordering or digest values (PKG-04)

**Plans**: 3/5 plans executed (3 sequential waves executed; 2 gap-closure plans added 2026-08-24, both wave 1, parallel)

Plans:

**Wave 1**

- [x] 121-01-PLAN.md — Regression net first: `pmcp-package` dev-dep + pin tripwire (D-01/D-03), the `test-openapi-server` Makefile gate with a nonzero-test-count guard (D-13), and the `tests/common/` helper lift with `mount_london_tube` parameterized by credential (D-02/D-12)

**Wave 2** *(blocked on Wave 1 — consumes the lifted helpers)*

- [x] 121-02-PLAN.md — TRACER + positive E2E: pack in A, move the OCI layout to a distinct B, unpack, serve both, compare `(name, inputSchema)` surfaces (D-07/D-10/D-11); `required_slots` set-equality against the hardcoded literal and `detect_deviation`'s drift role (D-04/D-06); scenario replay green in B with per-step gating

**Wave 3** *(blocked on Wave 2 — same file)*

- [x] 121-03-PLAN.md — SC4 both directions: negative tests over a deliberately-degraded environment B (D-08), the D-09 structural guard with a nonzero-lines-scanned floor, and the stale `detect_deviation` rustdoc correction

**Gap closure** *(added 2026-08-24 from `121-VERIFICATION.md` — all 4 success criteria VERIFIED; these close the 2 code-review BLOCKERs in the infrastructure that gates the deliverable. Both are wave 1 and run in parallel: disjoint `files_modified`, no shared state.)*

- [x] 121-04-PLAN.md — CR-01: the `pmcp-package` dev-dep drops its `version` key (path-only), so `cargo publish -p pmcp-openapi-server` at `release.yml:339` no longer requires an unpublished `pmcp-package 0.2.0`; the pin tripwire is re-pointed at the publish-safe shape plus the resolved crate's own 0.2 line, and the constraint is written into CLAUDE.md's publish ledger. `release.yml` order is deliberately NOT changed.
- [x] 121-05-PLAN.md — CR-02: the `REQUIRED_TEST_BINARIES` guard reads each binary's `test result:` passed count anchored to its `Running tests/<name>.rs` line via a single `scripts/named-test-binary-count.awk` extractor, with a five-fixture self-test chained into the same gate and a red/green demonstration against a real zero-test binary.

### Phase 122: Attestation Carriage *(contract-first — PARKED on the pmcp.run backend)*

**Goal**: A package can carry a **pmcp.run-issued** attestation and a verification path exists against pmcp.run's identity. The SDK's job is carriage and verification only — no signing, no crypto dependency added to `pmcp-package`, and `digest::verify` stays an integrity check rather than becoming a signature check. The in-repo half is a vendored contract plus an offline blocking contract test; the live issuance leg activates only if the backend is scheduled.
**Depends on**: Phase 120 (layer + vendor-media-type machinery); runs in parallel with Phase 123
**Requirements**: PKGX-01
**Parked**: yes — the critical path (attestation issuance on version promotion) is pmcp.run backend work outside this repo. Every criterion below is achievable offline with the backend unavailable.
**Success Criteria** (what must be TRUE — all offline, in this repo, backend unavailable):

  1. An attestation contract is vendored at `contracts/pmcp-run/attestation-v1.graphql`, a sibling of the existing `contracts/pmcp-run/capture-v1.graphql`, and an offline **blocking** contract test in `cargo-pmcp/tests/` validates the CLI's attestation operations against it with `apollo_compiler` — no network, no pmcp.run credentials, running in the default `cargo test` gate exactly like `cargo-pmcp/tests/package_capture_contract.rs` (PKGX-01)
  2. `pmcp-package` carries an attestation as an **opaque** layer under an `application/vnd.pmcp.*` media type: a package with an attestation and one without both round-trip through `pack_server`/`unpack_server` **and** through `pack_team`/`unpack_team` (D-08 — the unit that ships to prod is a team, and an agent is a team-of-one), and the crate never deserializes or interprets the attestation bytes. `pack_agent`/`pack_workflow` deliberately do **not** expose the attestation parameter despite sharing `pack_single_layer` (PKGX-01)
  3. `cargo pmcp package inspect` renders an attestation's presence, its subject digest and its issuer when one is carried, and reports the package as unattested when none is — driven entirely by fixtures, with no network call on any path (PKGX-01)
  4. The no-crypto boundary is machine-checked rather than stated: a dependency tripwire test (the `const + include_str! + assert` pattern `cargo-pmcp/tests/pmcp_package_pin.rs` already uses for version pins) fails if a signing or crypto crate enters `pmcp-package`'s dependency tree (scoping Decision 1)
  5. The parked boundary is explicit and reversible in one step: the live issuance/verification leg exists as an `#[ignore]`d, env-gated test that names exactly what the backend must ship — the `parity_live_tfl` / `PMCP_OPENAPI_LIVE_TEST=1` double-gate pattern — so promoting this phase from parked to blocking is removing a gate, not writing a new test (PKGX-01)
  6. Attestation implies resolved (D-09/D-10, the `cargo build --locked` analogue): attaching an attestation to a package holding any `ComponentRef::Range` fails at pack time with an error naming the offending component and its `component_type`, and `PinnedRef` gains `resolved_from: Option<VersionReq>` so pinning records the declared range instead of destroying it. The guard is explicitly **one level deep** — a test constructs attested team → pinned agent → agent holds a `Range` and asserts the team still packs, so the depth limit is pinned visible behaviour rather than an unexamined gap (PKGX-01)

**Plans**: 8/8 plans executed across 5 waves (planned 2026-08-25; tracer-first — 122-02 Task 1 is the end-to-end slice, verified before every expansion task). Revised 2026-08-25 after cross-AI review (`122-REVIEWS.md`): the attestation media type is settled as KIND-NEUTRAL `application/vnd.pmcp.attestation.v1` before the tracer freezes it (122-02 decision record), the parked live leg gains an executable request path plus an auth gate (122-04), and the pre-write subject gate gains the pure `describe_blob` extraction that makes it implementable (122-03).

Plans:

**Wave 1** *(parallel — disjoint files)*

- [x] 122-01-PLAN.md — Wave-0 infrastructure: the `test-cargo-pmcp-integration` gate-reach target (RESEARCH Pitfall 1 measured that `cargo-pmcp/tests/*` runs in NO gate) plus the no-crypto boundary — a generated crate-local cargo-deny allowlist and `make no-crypto-check` (D-12/D-13, SC4)
- [x] 122-02-PLAN.md — **TRACER**: attestation bytes attach at `pack_server`, travel as an opaque media-type-keyed layer with subject/issuer/payload-type in LAYER descriptor annotations, come back byte-identical from `unpack_server`, and render in `cargo pmcp package inspect` — all offline (D-01/D-04/D-05/D-14, SC2/SC3)

**Wave 2** *(parallel — disjoint files)*

- [x] 122-03-PLAN.md — The only offline verification: subject-digest comparison at both ends. Pack refuses before writing; unpack reports a mismatch as DATA; `inspect` renders the diagnostic then exits non-zero, quiet mode included (D-02/D-03/D-06)
- [x] 122-04-PLAN.md — Contract-first: SDK-proposed `attestation-v1.graphql`, `VERIFY_ATTESTATION_QUERY` plus a pure request-builder/response-decoder seam, the offline blocking `apollo_compiler` test, the `#[ignore]`+env-TRIPLE-gated parked live leg carrying an executable request path, and the platform ratification ask (D-07/D-11, SC1/SC5)

**Wave 3** *(parallel — disjoint files)*

- [x] 122-05-PLAN.md — Bounded format addition: `PinnedRef.resolved_from: Option<VersionReq>` (Cargo's range-plus-resolution model) and `TeamPackage`'s pinned-components guard, with the golden fixtures measured not to move (D-09 helper/D-10, SC6)
- [x] 122-06-PLAN.md — ALWAYS requirements: opacity and untrusted-annotation robustness as proptest properties, a runnable `attestation_carriage` example, and a nonzero-test-count assertion on `pmcp-package-gate`

**Wave 4**

- [x] 122-07-PLAN.md — Team carriage through the shared `pack_single_layer` helper, `unpack_team -> UnpackedTeam`, Gate A (attestation implies resolved) with the one-level depth limit as a passing test; `pack_agent`/`pack_workflow` deliberately unchanged (D-08/D-09, SC2/SC6)

**Wave 5** *(has a blocking `checkpoint:decision` — `autonomous: false`)*

- [x] 122-08-PLAN.md — Ratify and propagate the `pmcp-package` version this phase ships across all seven emitters, re-point both pin tripwires, and record the bump in CLAUDE.md's publish ledger. Phase 124 keeps the publish half.

### Phase 123: Export/Import Verbs *(contract-first — PARKED on the pmcp.run backend)*

**Goal**: `cargo pmcp package` grows the portability verbs — `save` and `load` are the local file round trip and land immediately; `pull` resolves its environment through `configure`'s existing resolver and reuses the working `deployment/targets/pmcp_run/{graphql,auth}.rs` seam rather than inventing a second API path, and is contract-first against the platform's `getPackageArtifact` contract. `export` is retired and `import` stays the platform's, unchanged.

> **Verb set restated 2026-08-26 (supersedes the `pack | unpack | export | import` wording above and throughout this section).** The platform exchange settled all four names: `pack`/`unpack` became **`save`/`load`** (Docker's local-file split); `import` **stays the platform's**, meaning *admit a package into an environment*; `export` is **retired** — it was `push` under another name and `capture` already produces packages platform-side, so it had no defensible job. **`pull`** takes the remote slot. Authoritative record: `123-CONTEXT.md` D-01/D-02/D-03 and `docs/platform-requests/attestation-carriage-sdk-reply.md` §6(a).
**Depends on**: Phase 120 (the pack/unpack package shape); runs in parallel with Phase 122
**Requirements**: PKGX-02
**Parked**: partially — `pack`/`unpack` are unblocked and land this milestone; `export`/`import` are contract-first, their live leg parked on pmcp.run package import.
**Reality check 2 (2026-08-26, from the pmcp.run exchange — READ BEFORE PLANNING)**: two of this phase's four declared verbs changed meaning, and the verb count in Reality check 1 is itself measured against the wrong branch.

  - **`import` is NOT ours to take.** The platform answered the collision question: `import` stays theirs. It is not merely a CLI verb — it is `submitImport`/`getImportStatus` on their AppSync API, four data models, the Phase 173.5 admin UI, an ADR and a live D-14 acceptance. SC2's "resolve the collision" is answered: we do not rename theirs, we name ours differently.
  - **`pack`/`unpack` become `save`/`load`**, per Docker's split (`save`/`load` local file, `push`/`pull` registry, `import` admit-into-system). Verified free on both branches, as are `push`/`pull`. `install` is excluded — their Phase 184 admin UI already uses "Install App". SC1 should be restated in these terms.
  - **⚠ `export` has no defined job left, and this phase must decide its fate.** It was specified as a *remote* op alongside `import` (SC3/SC4), i.e. the inverse of `pull` — which is `push`. But `capture` already produces packages platform-side and `pull` (§5.1) covers the other direction. The platform asked directly: "if `export` is a new verb, what does it do that `capture` doesn't?" We have no defensible answer. **Analysis says drop it**; doing so changes this phase's declared verb set and SC3/SC4, so it is a planning decision, not a documentation fix.
  - **The five-verb count in Reality check 1 is branch-local.** `feat/package-172-cli` carries `f7ea3c4b` and `3425662d` (both 2026-07-21): `PackageCommand` has **eight** variants there and its `Import` is real, not dry-run. SC2's `verb_help.rs` pin would encode a list contradicting the platform's live control plane. **The platform asks that this branch merge BEFORE the verb list is pinned.** Their own qualifier: 172-10 was blocked before `activate` ever ran, so `activate`/`rollback`/`cancel` are wired but not exercised end to end — the test asserts the inventory, not the acceptance.
  - Full context: `docs/design/package-portability-pmcp-run-handoff.md` §5.2, and the three-message exchange in `docs/platform-requests/attestation-carriage-*.md`.

**Reality check (measured 2026-08-22)**: the scoping line "`cargo pmcp package` today has exactly one verb: `inspect`" is stale. `cargo-pmcp/src/commands/package/mod.rs` on `main` enumerates **five** — `inspect | capture | show | import | approve` — and `import` is already taken by the remote *workflow-manifest dry-run* import (`cargo-pmcp/src/commands/package/import.rs`). The AI-Package import verb therefore collides with an existing, shipped verb, and this phase must resolve that explicitly.
**Success Criteria** (what must be TRUE — all offline, in this repo, backend unavailable):

  1. `cargo pmcp package save` and `cargo pmcp package load` work fully offline — `save` reads a `pmcp-openapi-server` `config.toml` plus `.pmcp/deploy.toml` and writes a **movable `.tar` file** (server kind only), `load` reads that tar back into a **working OCI layout directory** that `inspect` opens unchanged and prints the slots the target environment must fill (kind-agnostic) — and both appear in `cargo pmcp package --help`, asserted by `cargo-pmcp/tests/verb_help.rs` (PKGX-02)
  2. The collision with the existing remote `package import` verb is resolved — externally, on 2026-08-26: `import` stays the platform's with its meaning unchanged, and ours are `save`/`load`/`pull`. This phase asserts the CONSEQUENCES: a group `--help` preamble names the three directions and is asserted by a test, no already-shipped verb changes meaning, and `verb_help.rs` pins the complete post-change verb list by set equality against one documented constant. Two qualifiers are part of the criterion: the pin covers the set **on this branch** and **breaks by design** when `feat/package-172-cli` merges, and it asserts the **inventory, not the acceptance** — `activate`/`rollback`/`cancel` are wired there but never exercised end to end (PKGX-02)
  3. The **`pull`** path resolves its environment through `configure`'s existing resolver and the `pmcp_run` seam — `get_api_base_url()`'s `PMCP_API_URL` precedence (`cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:113`) and the TTL'd, endpoint-keyed config cache (`auth.rs:201`) — with **no second API path**: no new base-URL environment variable, no second token cache and no second `reqwest::Client`, checkable by grep over `cargo-pmcp/src/` (PKGX-02)
  4. The platform half is contract-first and offline-blocking: **`getPackageArtifact`** is validated against the vendored `contracts/pmcp-run/portability-v1.graphql` by a test in the default `cargo test` gate, and invoking `pull` with no reachable backend fails with a message naming the missing platform capability rather than a raw transport error, with the underlying cause preserved in the `anyhow` chain (PKGX-02)
  5. The artifact's **tar framing rule** — entry inventory, no wrapper directory, no absolute or `..` paths, no symlinks, no duplicates, uncompressed, reproducible headers — is documented normatively in `crates/pmcp-package` (the crate both sides read) and pinned by checked-in golden fixtures that the writer under test never regenerates; and a `save`/`load`/`pull` whose artifact fails verification leaves the destination **byte-for-byte unchanged** (PKGX-02)

**Plans**: 7/7 plans executed

Plans:
**Wave 1**

- [x] 123-01-PLAN.md — Tracer: the `save` → `load` artifact spine (tar codec, validated descriptor-graph model, framing gates, injectable byte caps, staged transactional install)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 123-02-PLAN.md — Contract-first `getPackageArtifact`: vendored SDL, pure codec, offline blocking test, parked live leg
- [x] 123-03-PLAN.md — `load`'s report (slots, three-state pin facts, carriage states, exit-1 subject mismatch), the renderer lib mount, and `save`'s scope guards

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 123-04-PLAN.md — The tar framing rule in `pmcp-package` docs, its golden-fixture corpus, and the tests binding the rule to BOTH the reader and the writer

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 123-05-PLAN.md — `pull`: the six-stage pipeline in a lib-mounted module behind one transport seam, the module-scope HTTP client, and D-05's named-capability failure

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 123-06-PLAN.md — The verb-list pin plus its gate registration, the `--help` three-direction preamble, and the written note to the platform

**Wave 6** *(blocked on Wave 5 completion)*

- [x] 123-07-PLAN.md — ALWAYS coverage (fuzz target, round-trip example) and the end-to-end proof that the gate executes all eight named test binaries

> **Wave layout revised 2026-08-26** by `/gsd-plan-phase 123 --reviews`, from five waves to six. Two
> causes, both from the cross-AI review: each creator plan now registers its own test binary in
> `test-cargo-pmcp-integration` in the same commit that creates it (`Makefile:337-339`'s own
> discipline — deferring all four to plan 07 left Waves 1-4 running a gate that executed none of their
> own new tests), and same-wave plans must not share `files_modified`, so the four `Makefile` editors
> are spread one per wave. Plan 04 also gained a real dependency on plan 02, whose vendored SDL its
> framing-rule docs cross-reference. Plan numbering is unchanged.

### Phase 124: Release & Publish Order

**Goal**: What this milestone builds actually ships. The publish ledger names every crate it must, the machine-checked half of that ledger covers the crate this milestone bumps, and the version pins between `pmcp-package` and `cargo-pmcp` move together so the CLI can never ship pinned to a package version it cannot read.
**Depends on**: Phases 120-123 (publishes what they land)
**Requirements**: PKGR-01
**Reality check (measured 2026-08-22)**: PKGR-01's premise — "`pmcp-openapi-server` is absent from CLAUDE.md's publish order (zero occurrences)" — was true at scoping and is **no longer true on `main`**. The crate holds slot 9b in CLAUDE.md, has a publish step in `.github/workflows/release.yml`, and `scripts/check-release-coverage.sh` (wired into `.github/workflows/ci.yml:233`) machine-checks the workflow half. Two residual gaps are this phase's real work: (a) the gate enumerates crates via `cargo metadata --no-deps`, so it structurally cannot see workspace-**excluded** publishable crates — and `crates/pmcp-package`, the crate this milestone bumps, carries its own `[workspace]` table (`crates/pmcp-package/Cargo.toml:6`) so it is not a root member at all — measured: `cargo metadata --no-deps` lists 28 packages and `pmcp-package` is not among them; (b) the version numbers named at scoping are stale — `cargo-pmcp` is at 0.21.0 (not 0.19.0) and `pmcp-package` at 0.1.1.
**Success Criteria** (what must be TRUE):

  1. On the milestone branch — 249 commits divergent from `main`, so this is re-verified rather than assumed — `./scripts/check-release-coverage.sh` exits 0 and `pmcp-openapi-server` appears in both CLAUDE.md's publish order and `.github/workflows/release.yml` (PKGR-01)
  2. The coverage gate is extended to workspace-excluded publishable crates so `crates/pmcp-package` is covered, and deleting its `cargo publish --manifest-path crates/pmcp-package/Cargo.toml` step from `release.yml` makes the gate fail — demonstrated by running it, not asserted in prose (PKGR-01)
  3. `pmcp-package` ships a new version carrying the Phase 120 layer and media-type additions, and `cargo-pmcp`'s caret pin plus the `cargo-pmcp/tests/pmcp_package_pin.rs` tripwire move in the same change, so the CLI cannot ship pinned to a `pmcp-package` version that cannot read the packages it writes. Targets are the next bump from HEAD, not the stale 0.2.0 / 0.19.0 pair named at scoping (PKGR-01)
  4. CLAUDE.md's publish order and `release.yml`'s step order agree that `pmcp-package` precedes `pmcp-cfn-renderer`, `pmcp-agent` and `cargo-pmcp`, with the constraint stated in the ledger rather than left implicit in step ordering (PKGR-01)

**Plans**: 7 plans across 7 waves (1:{01} 2:{02} 3:{03} 4:{04} 5:{05} 6:{06} 7:{07}), all carrying PKGR-01.

Plans:
**Wave 1**

- [ ] 124-01-PLAN.md — Coverage gate sees the workspace-excluded crate: D-01 two-source discovery plus a repo-wide scan-scope tripwire, D-10 cluster order assertion with a word-boundary matcher, eight-fixture red-direction self-test declared as the gate's prerequisite

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 124-02-PLAN.md — Sync `main` into the milestone branch (D-08), resolving the squash-merge conflict set BEFORE any version bump

**Wave 3** *(blocked on Wave 2 completion)*

- [ ] 124-03-PLAN.md — D-05 three-way version-drift sweep as a committed tool, D-03's public-API guard, and a blocking-human decision on every phantom delta

**Wave 4** *(blocked on Wave 3 completion)*

- [ ] 124-04-PLAN.md — D-09 ledger reconciliation: release.yml's four stale comment regions and CLAUDE.md's scattered ordering constraint plus its Pre-Flight contradiction

**Wave 5** *(blocked on Wave 4 completion)*

- [ ] 124-05-PLAN.md — Version bumps for exactly the authorised set in one atomic commit, the D-04 pmcp-package audit with its nine-emitter one-set rule plus the separate row-C1 consequence set, the CHANGELOG section the release workflow can extract, and the committed `124-expected-release.json` release manifest that plans 06 and 07 verify against

**Wave 6** *(blocked on Wave 5 completion)*

- [ ] 124-06-PLAN.md — Open the release PR, drive CI green, human-action checkpoint for the merge

**Wave 7** *(blocked on Wave 6 completion)*

- [ ] 124-07-PLAN.md — Pre-tag re-verification of two disjoint sets (expected-new vs expected-skips), one-way-door decision checkpoint, human-action tag push, per-crate registry verification with retry, and a closeout PR that delivers the planning record to `upstream/main`

> **Waves are one plan wide by design.** This is a release pipeline: measure -> gate -> sync -> reconcile -> decide versions -> ship, and every step's output is the next step's input. Two ordering constraints are load-bearing and must not be "optimised" back into parallelism: the `main` sync (plan 02) precedes every version bump, because a conflict resolution that silently reverts a bumped manifest is the worst possible ordering; and the phantom-delta decision (plan 03) precedes the bumps it authorises, because published version numbers are consumed permanently.

## Progress — Current Milestone

*Milestone **v2.6 AI-Package Portability** — Phases 120-124, opened 2026-08-22.*

**Execution order:** 120 → 121 first and together (no external dependency, and 121 is the regression
net every later refactor leans on) → 122 and 123 in parallel, both contract-first → 124 last.

**Requirement coverage:** 7 of 7 v2.6 requirements mapped to exactly one phase each, 0 unmapped, 0
duplicated. Authoritative table: `.planning/REQUIREMENTS.md`.

| Phase | Requirements | Plans Complete | Status | Completed |
|-------|--------------|----------------|--------|-----------|
| 120. Config-Server Packaging | PKG-01, PKG-02, PKG-03 | 5/5 | Complete    | 2026-08-23 |
| 121. Local Round-Trip E2E | PKG-04 | 5/5 | Complete    | 2026-08-25 |
| 122. Attestation Carriage *(parked)* | PKGX-01 | 8/8 | Complete    | 2026-08-25 |
| 123. Export/Import Verbs *(parked)* | PKGX-02 | 7/7 | Complete    | 2026-08-26 |
| 124. Release & Publish Order | PKGR-01 | 0/7 | Planned | - |

> **⚠ Phases 122 and 123 cannot fully close inside this repo.** Both depend on pmcp.run backend
> capabilities — package import and attestation issuance — that were not confirmed as scheduled at
> milestone scoping. They are planned contract-first so the in-repo half is completable and
> verifiable offline. If the backend work is scheduled, promote them from parked to blocking and
> add the live E2E leg.

## v2.7 SEP-2640 Skills Conformance & Positioning (Phase 125+)

**Milestone Goal:** Bring the shipped `skills` module (Phase 80, feature `skills`) into
conformance with the CURRENT SEP-2640 draft (PR #2640 head `sep/skills-extension`, rewritten
2026-08-29), then land the positioning work spikes 009-011 validated: workflow→skill projection,
digest-pinned agent skill consumption, and the tri-surface decision-matrix docs. Grounded in
validated spikes 008-011 (`.planning/spikes/`) and the spike-findings skill
(`.claude/skills/spike-findings-rust-mcp-sdk/` — references `sep-2640-conformance.md` and
`skills-positioning-tri-surface.md`).

**Scoping note:** Phase 125 opens the milestone (the CRITICAL conformance fix — the shipped
module declares a capability it cannot answer). Later phases (workflow→skill projection
`as_skill()`, `[[skills]]` digest pins on AgentPackage, decision-matrix docs) are to be added
via `/gsd-phase add` as they are taken up — implementation-order items 23-25 in the
spike-findings skill.

- [ ] Phase 125: SEP-2640 Conformance — skills/list + skills/get

### Phase 125: SEP-2640 Conformance — skills/list + skills/get

**Goal**: A pmcp server that declares `io.modelcontextprotocol/skills` actually answers it. The current SEP-2640 draft makes `skills/list` and `skills/get` mandatory for any server declaring the extension; the shipped module auto-declares and implements neither, so a conforming host's first `skills/list` call gets -32601. This phase routes both methods via the crate-private `InternalClientRequest` classifier (pattern at `src/types/protocol/mod.rs:583` — NO new public `ClientRequest` variant, 2.x promise), answers them entirely from the shipped `Skills` registry with conforming entries (verbatim frontmatter JSON + complete `{uri, digest: sha256, size}` manifests), retires (or legacy-gates) the nonstandard `skill://index.json`, validates name-identity (final URI segment == frontmatter `name`) at build, and guards the ≤512-file / ≤16 MiB limits at `into_handler()`.
**Depends on**: Nothing in-repo (first phase of the milestone). Spike 008
(`.planning/spikes/008-sep-2640-drift-check/`) is the measured drift evidence; the fix contract
lives in `.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md`.
**Requirements**: TBD — no formal REQ-IDs; the tracked requirement set is `125-CONTEXT.md`
decisions D-01..D-11 (all 11 covered by the plans below; gate `check.decision-coverage-plan`
reports 11/11).
**Plans:** 5 plans across 4 waves — 1:{01} 2:{02,03} 3:{04} 4:{05}

Plans:
- [ ] 125-01-PLAN.md — TRACER: `skills/list` end-to-end over streamable HTTP + the SC#1 routing guarantees
- [ ] 125-02-PLAN.md — `skills/get` with draft-correct -32602 semantics + `ServerCore` twin-site parity
- [ ] 125-03-PLAN.md — Complete `resources` manifests, verbatim frontmatter, D-02 warn+exclude, name-identity reject, SEP limits warning
- [ ] 125-04-PLAN.md — Retire `skill://index.json` (12 tracked sites) + examples and the four documentation surfaces
- [ ] 125-05-PLAN.md — `make test-skills` gate leg with a zero-test-count guard, fuzz target, and the rustdoc deferral record

**Success Criteria** (what must be TRUE):

  1. `serde_json::from_value::<ClientRequest>` on `{"method":"skills/list"}` still returns Err
     (no new public variant — 2.x exhaustive-enum promise), yet a built server with registered
     skills answers `skills/list` and `skills/get` wire requests with entries carrying verbatim
     frontmatter JSON and complete `{uri, digest: sha256:{64 lowercase hex}, size}` manifests
     (spike 008 gaps #1, #2).
  2. `skill://index.json` is no longer served by default once `skills/list` lands — retired or
     behind an explicit legacy gate (gap #3).
  3. `Skills` build-time validation rejects a skill whose final URI segment does not equal its
     frontmatter `name`, and warns when a skill exceeds 512 files or 16 MiB (gaps #4, #5).
  4. Existing conforming behavior is unchanged: SKILL.md + supporting files remain byte-identical
     via `resources/read`, supporting files stay out of `resources/list`, the dual-surface prompt
     fallback still holds byte-equality, and examples s44/c10 still pass.
  5. `resources/directory/read` (gap #6) and client wrappers (gap #7) are explicitly deferred or
     implemented — never silently dropped; the current `{}` declaration legitimately means
     `directoryRead: false`.
