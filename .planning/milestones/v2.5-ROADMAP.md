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
- 📋 **v2.5 MCP Spec 2026-07-28 (v2) Support** — Phases 112-119 (planned)

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

## Phase Details — Current Milestone

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

## Progress — Current Milestone

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

## v2.5 MCP Spec 2026-07-28 (v2) Support (Phases 112-119)

**Milestone Goal:** Make pmcp a dual-version SDK that transparently serves both MCP 2025-11-25 and the 2026-07-28 (v2) spec from one binary via per-request negotiation — a stateless core, Tasks-as-extension, JSON Schema 2020-12, auth-hardening SEPs, and conformance against the official suite. v2 is the strategic primary path (stateless/Lambda-first) and v1 is a cleanly severable compatibility layer; the whole milestone stays a 2.x minor (additive, no 3.0).

**Dependency spine (research-corroborated, all four passes):** version-plumbing spine (VERS) is the keystone and lands first and alone — nearly every other v2 behavior era-gates off it. Stateless HTTP + MRTR next (Tasks has a loose dependency on it for the shared stateless-identity/owner-binding pattern). JSON Schema and Auth parallelize with the HTTP/Tasks track once the spine lands. Client/agent tooling + v1 severability ride after the v2 server paths exist. Conformance is last (validates the union). Docs close it out.

**Final-spec checkpoint:** The 2026-07-28 spec finalizes six days after roadmap creation. Wire-exact work (error-code values, `requestState` shape, caching-hint field names) is sequenced so it lands after final publication — VERS-06's error-code table is structure-first, values-from-final-schema.json only.

- **`-32002` open verification item — RESOLVED (Phase 113, plan 01; re-confirmed plan 12).** The draft schema's error-code block states verbatim that codes from earlier protocol versions "remain reserved and are never reused: `-32002` (**resource not found**, 2025-11-25 and earlier; replaced by `-32602`) and `-32042` (URL elicitation required, 2025-11-25 only)". The `-32002`→`-32602` rename therefore targets **resource-not-found, NOT task-pending**. pmcp's proprietary `V1_TASK_PENDING` squat on `-32002` is **unaffected and stays frozen**, exactly as Phase 112 decided when it kept both `-32002` meanings by name (`V1_TASK_PENDING` vs `UNSUPPORTED_CAPABILITY`). **Phase 114 must not re-litigate this.** Evidence: `113-SPEC-RECHECK.md` § A.4, against `schema/draft/schema.ts` @ `71e3069`. *(Caveat: read from the DRAFT — the final schema had still not published as of 2026-07-26, so this resolution is re-checkable but not yet final.)*
- **⚠️ UNASSIGNED requirement — SEP-2243 `x-mcp-header` / `Mcp-Param-{Name}` (UNAS-01).** The v2 transport spec says clients **MUST** support `x-mcp-header` mirroring and its header-mismatch validation table covers `Mcp-Param-*`. **No requirement in this milestone covers it** — not VERS-05, not HTTP-01..05, not CLNT-01 — and no Phase-113 plan implements it (113-RESEARCH A8 / Open Question 4 both resolved explicitly *not* to absorb it into 113). It is closest to CLNT-01's header work and to the Phase-112 `classify_v2_request` matrix. **Needs a phase assignment**; recorded in `.planning/REQUIREMENTS.md` § "Unassigned — Awaiting Phase Assignment".

**Non-goals:** Hard cutover to v2 (dual-version now, sunset later per SMPL-01); removing Roots/Sampling/Logging (deprecated with a 12-month advisory window — CONF-03 runtime verification only); adding `oauth2`/`openidconnect` crates (auth SEPs land as source changes). Zero new runtime dependencies — only `jsonschema` 0.46→0.48 for Draft 2020-12; Node.js LTS 22.x is CI-only for the conformance suite.

- [x] **Phase 112: Version Plumbing Spine** — `ProtocolContext` resolved once at ingress + threaded through dispatch; 2026-07-28 as explicit opt-in (LATEST stays 2025-11-25); `server/discover`, extensions map, required v2 headers, `resultType` envelope, W3C trace-context, centralized version-gated error-code table (completed 2026-07-23)
- [~] **Phase 113: Stateless HTTP + Multi-Round-Trip Elicitation** — v2 requests run handshake-free/session-free on the existing `stateless()` branch; MRTR (`input_required`/`requestState`/`inputResponses`) end-to-end; opt-in `subscriptions/listen` (server + client halves); no SSE resumability + id-replay regression test; the pmcp `Client` speaks v2 and fulfills MRTR. **All 20 plans shipped, but re-verification on 2026-07-26 returned `gaps_found` (4/5 must-haves)** — the original 13, plus the four-plan gap-closure round (113-17 SSE parser bound / 113-18 listen refusal + semaphore prune / 113-20 collected-body cap / 113-19 fuzz-seam gating + phase gate) that answered `113-VERIFICATION.md`'s GAP-A..E. **The progress table's "Complete" for this phase means all 20 plans shipped; the phase itself is NOT complete, for two independent reasons:** (1) **Open codebase gaps.** A fresh `113-REVIEW.md` (2026-07-26) found three BLOCKERs that the gap-closure round neither introduced nor closed, all independently reproduced by re-verification: an uncapped `body.collect()` in `rejection_error` (`src/client/subscriptions.rs:147`), an O(n²) `take_utf8_prefix` upstream of every new bound (`src/shared/sse_parser.rs:164`), and an uncapped `response.collect()` in `HttpTransport::send_request` (`src/shared/http.rs:346`). These leave HTTP-04's memory-bounded-stream criterion genuinely unmet. (2) **Blocked on publication — ✅ DISCHARGED 2026-08-18 by Phase 119 task zero (plan `119-01`).** The final `schema/2026-07-28` had still not published as of 2026-07-26, so the three v2 error-code constants were pre-final values held under a written developer exception and HTTP-01..08 / CLNT-01/02/05 stayed `[~]`. That is now closed: the versioned directory exists upstream, its two blobs are byte-identical to the vendored pin (`9b55feeb…`/98426, `213c58f6…`/181474), **both arms** of the re-verification obligation were run and recorded, `113-SPEC-RECHECK.md`'s `## Verdict` reads **`PUBLISHED-CONFIRMED`**, and all **eleven** requirements are flipped `[~]` → `[x]`. The run record is `113-SPEC-RECHECK.md` `### Verdict re-verification — Phase 119 task zero (2026-08-18)`. **(1) is UNTOUCHED and still needs a further gap-closure round** — Phase 119 is a documentation phase and changes no `src/` code, so this phase entry stays `[~]` on the strength of (1) alone. Closing (2) does not close the phase.
- [x] **Phase 113.1: Merge Unblock** — the three blockers that kept this branch from merging are CLOSED. (1) **PR-blocking PMAT complexity**: `handle_post_fast_path` 30 → **15** and `handle_post_with_middleware` 31 → **15** (pmat 3.15.0, measured per-function), so `pmat quality-gate --fail-on-violation --checks complexity` passes **locally** with zero violations — five points of margin under the phase's ≤ 20 target. Delivered by extracting the copy-pasted v2 header gate into a `resolve_v2_gate` sibling pair, the fast path's inline dispatch into `dispatch_message_fast`, a read/classify preamble per path, and a legacy-version guard per path preserving D-08's deliberate asymmetry. (`FastPathDispatch`/`MiddlewareDispatch` remain field-for-field identical and unmerged — D-07, still deferred.) (2) **D-113-R**: `drain_complete_lines` now searches from a scan-window cursor instead of restarting at 0 each `feed()`, with the per-call `debug_assert` full-buffer scan removed in the same atomic change; guarded by two falsifiable O(n) tests, each demonstrated RED before the fix and RED again under a post-fix negative control, plus a chunking-invariance property. (3) **D-113-Q**: `OptimizedSseTransport::connect_sse` is bounded by a `reqwest::chunk()` running total against the crate's 16 MiB SSE ceiling, and `WHOLE_BODY_ALLOWLIST` is now **EMPTY** — the ratchet at its floor. **HTTP-09 is `[x]`, closed on the merits** with its requirement text byte-unchanged. The auth-surface reads are recorded as `D-113-V` and assigned to Phase 116 — **four** files, not three, and **31** reviewed-unbounded reads, not 18 — the original figure was a raw line-grep count; the tripwire's own scanner strips whitespace and would find all of them, so only its SCOPE FENCE keeps these four files unreported. **Still outstanding for merge, neither caused nor owned by this phase:** the org-required `gate` needs a human push (D-20), and two pre-existing CI failures stand in front of it — `make doc-check`'s 26 rustdoc errors (`D-113-W`) and the Purity Gate's tooling drift.
- [~] **Phase 114: Tasks Extension Migration** — Tasks negotiated via the extensions map, `tasks/update` added, `tasks/list` era-gated off on v2; `resultType:"task"` + 5-state→v2-enum mapping; stateless owner-binding fails closed; TaskStore/backends survive unchanged (wire reshape behind TaskRouter). **All 20 plans shipped (2026-08-01) and the whole-phase gate is GREEN** — `make quality-gate` exit 0 at 4899 passed / 0 failed across 294 result lines, `make lint` zero warnings, semver 223/223 no-update-required against the phase base, `cargo public-api` 0 REMOVED, pmat 0 violations in `src/`, both examples exit 0, fuzz 20 000 runs clean, `Cargo.lock` byte-unchanged across the phase. **The phase is nonetheless NOT complete, for two independent reasons, exactly as Phase 113's marker records for its own:** (1) **`114-18`'s Task 4 sign-off checkpoint (`checkpoint:human-verify gate="blocking"`) has NOT been answered** — it is a reserved human action and was not self-approved. (2) **Blocked on publication.** `114-SPEC-RECHECK.md` `## Verdict` is `PENDING` and TASK-01..06 are booked `[~]` — *implemented; pending final schema* — under the **D-18** hold. Re-measured 2026-08-01T00:09:19Z with the prescribed `gh api` form: `modelcontextprotocol/modelcontextprotocol` HAS published `schema/2026-07-28/`, but `modelcontextprotocol/ext-tasks` still carries `draft/` only with 0 tags and 0 releases. Under DQ6's both-repositories trigger that is a partial publication, which the record's `## Third Outcome Policy` rule 5 defines as `STILL-ABSENT`. **The sole remaining condition is a ONE-repository check on `ext-tasks`; nothing watches it (`D-114-S`).**
- [x] **Phase 115: JSON Schema 2020-12 + Structured Output + Caching Hints** — jsonschema **0.49** (not the 0.48 the requirement text names) with Draft 2020-12 explicitly pinned on v2 via normalize-then-compile — *the naive pin is a MEASURED silent validation bypass* — proven wasm-clean by `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"`, which is the only command that compiles `jsonschema` at all (`make wasm-build` never does), and SEP-2106 fenced against cargo's DECLARED and RESOLVED dependency graphs; `structuredContent` accepts any JSON value on v2 via `CallToolResult::structured_value`, with v1's pre-existing over-permissiveness FROZEN by D-05 rather than corrected (there was no object-only guard in pmcp to remove — measured); additive `ttlMs`/`cacheScope` on **six** results, not five, because `DiscoverResult extends CacheableResult` in the pinned schema — ensured on v2 and STRIPPED on v1 at one shared chokepoint wired into all THREE dispatchers including the wasm one. **All 11 plans shipped (2026-08-01) and the whole-phase gate is GREEN over the SWEPT tree** — `make quality-gate` exit 0 at 5045 passed / 0 failed / 81 ignored across 309 result lines, `cargo semver-checks` 223/223 *no semver update required* against phase base `acd23b64`, `cargo public-api` **+188 added / 0 REMOVED** with `Cacheable`/`project_caching_hints`/`fuzz_support` correctly invisible, pmat 0 violations in `src/`, the fuzz target 660 271 runs with an EMPTY artifacts dir, and `examples/s52_v2_caching_hints` RUN (not merely built) to exit 0. **SCHM-01/02/03 are booked `[x]`, NOT `[~]` — D-15's contingency did not fire** and Phase 114's publication hold is NOT inherited: this phase's wire values come from the PUBLISHED core schema vendored at `schema/vendored/core-2026-07-28/` @ `271ecc9accafdd9b83a3c869fa67c22953b2af80`, digest-fenced by `tests/vendored_schema_provenance.rs`. **Signed off by Guy Ernest (owner) on 2026-08-01** at `115-10`'s Task 3 `checkpoint:human-verify gate="blocking"`, which was returned UNANSWERED rather than self-approved; no completion marker existed on disk before that answer (completed 2026-08-01). **⚠ REOPENED 2026-08-01 — `[x]` → `[~]`, and the green-gate claims above must be read with this correction.** The sign-off predates `115-REVIEW.md`, which landed minutes later and was confirmed by `115-VERIFICATION.md` (status `gaps_found`, **3/4** must-haves): **SCHM-01's pin is incomplete.** `normalize_schema_dialect` normalizes only the ROOT `$schema`, so a legacy dialect declaration on an embedded schema resource (a subschema carrying `$id`) survives it and produces the vacuous accept-everything validator the pin exists to prevent — measured twice independently, including `root-draft07 + embedded (v1,v2) = (Violates, Conforms)`, i.e. **v2 validating weaker than v1**. The gate was green and the fuzz campaign ran 660 271 times because all three defensive layers structurally exclude the triggering shape (`normalization_cases()`, `arb_schema_document()`, `is_dialect_neutral`). SCHM-02 and SCHM-03 were re-measured against the codebase during verification and **do** hold; only SCHM-01 is downgraded to `[~]`. **✅ GAP CLOSED 2026-08-01 by the two gap-closure plans `115-12` + `115-13`** — executed as option **(a)** of `115-VERIFICATION.md` § *Human Verification Required* (a closure plan implementing the recursive-normalization fix), NOT as option (b), an override, so the owner's `115-10` sign-off is expressly not read as covering CR-01. `115-12` made `normalize_schema_dialect` rewrite EVERY string-valued `$schema` at any depth behind the unchanged `Cow`-returning signature, skipping `const`/`enum`/`default`/`examples` payloads, and the three-row measurement re-run post-fix through the same seam now reads **`root-draft07 + embedded (v1,v2) = (Violates, Violates)`** — v2 is no longer weaker than v1, with row 1's v1 column deliberately unmoved (D-01). `115-13` widened the two generators that structurally could not reach the shape (`arb_schema_document()` now emits `$id`-bearing embedded resources — 100 of 256 cases carried an embedded non-2020-12 declaration; the fuzz target gained TOTAL invariant 5 plus `$defs`/`$id`/sole-key-`$ref` in the neutrality allowlist and two committed seeds, 13 total) and ran the whole-phase gate: `make quality-gate` exit **0** at **5052 passed / 0 failed / 81 ignored across 309 result lines**, `pmat quality-gate --checks complexity` **0 violations**, SCHM-02/03's seven binaries unregressed at **78/78**, a `-max_total_time=300` `+nightly` campaign of **3 951 202** runs with an EMPTY artifacts dir, and both generators OBSERVED to fail against a deliberately reverted root-only normalizer. SCHM-01 is re-booked `[x]` in `.planning/REQUIREMENTS.md` on that post-fix evidence, with the downgrade record amended rather than deleted. Re-verification is `/gsd:verify-phase 115`'s job; this phase marker deliberately stays `[~]` until that re-run scores the closure. **⚠ AND THE SAME GAP REOPENED A SECOND TIME, 2026-08-02 — `115-13`'s closure was itself premature.** `115-VERIFICATION.md`, re-run against the closure, falsified it by renaming a single `$defs` key: `115-12`'s recursion was POSITION-BLIND, testing `DATA_ONLY_KEYWORDS` against EVERY object key, so an `$id`-bearing embedded resource filed under a `$defs` entry an author had NAMED `default` was visited by neither walker and kept its legacy `$schema` — `$defs.default` measured `(Conforms, Conforms)` with `rewritten=false` (so no `tracing::warn!` fired either), against the control `$defs.Inner` → `(Conforms, Violates)`, `rewritten=true`. **✅ CLOSED AGAIN 2026-08-02 by `115-14` + `115-15`**, again as option **(a)** of the verification report's *Human Verification Required* item and NOT as an override. `115-14` landed `SUBSCHEMA_MAP_KEYWORDS` — a three-way member dispatch in BOTH walkers, so the values of a `properties`/`patternProperties`/`$defs`/`definitions`/`dependentSchemas` map are descended into unconditionally and those maps' own keys are never keyword-filtered — post-fix `$defs.default` → **`(Conforms, Violates)`, `rewritten=true`**. `115-15` closed the STRUCTURAL half: all three prior fences RESTATED the code's own rule (the postcondition called the crate's own blind detector; the property generator hard-coded the name `"Inner"`; fuzz invariant 5's collector re-implemented the same filter while its doc claimed the scan was "TOTAL" and "INDEPENDENT"), so a defect in that RULE was invisible to every one of them — MEASURED as a postcondition passing vacuously with `owned=false` and the detector reporting `None` on a document that still carried a legacy declaration. The repair is **rename invariance**, a metamorphic relation DERIVED from a 2020-12 spec fact (subschema-map keys are semantically inert author-chosen names, so normalizing an entry cannot depend on the name it is filed under) and consulting no keyword list at all, landed in BOTH generators plus seed `14_defs_named_default`. **Three negative controls observed**, including the decisive one: with BOTH restated copies of the rule ALSO made blind — so invariants 2 and 5 pass vacuously exactly as they did pre-`115-14` — seed 14 still exits 1, naming **invariant 6**. Gate over the fixed tree: `make quality-gate` exit **0** at **5054 passed / 0 failed / 81 ignored across 309 result lines**, `pmat quality-gate --checks complexity` **0 violations**, SCHM-02/03 unregressed at **78/78**, `-runs=0` replay clean over 14 committed seeds and a **3 697 874**-run `+nightly` campaign clean with an EMPTY artifacts dir, no `Cargo.toml`/`Cargo.lock` in the closure diff and **0** new `pub` items under `src/`. SCHM-01 re-booked in `.planning/REQUIREMENTS.md` AFTER those commands ran, with both prior records amended rather than deleted. **The phase marker STILL stays `[~]`** — scoring the closure is `/gsd:verify-phase 115`'s job, not this plan's **⚠ AND A THIRD COMPLETENESS GAP, 2026-08-02 — this one in the LIST, not the RULE.** `115-REVIEW.md` CR-01 found `SUBSCHEMA_MAP_KEYWORDS` was a FIVE-entry allow-list omitting `dependencies` — draft-04..2019-09's own map-from-instance-property-NAME-to-subschema keyword, which this module's own test comment already recorded (`D-115-03-C`) as still honoured by `jsonschema` 0.49.2 under the pin. `115-15`'s `[x]` was ACCURATE for the five keywords it measured; what moved is the LEVEL of the failure, and every fence `115-15` built enumerated that same constant, so an omission FROM it was invisible to all of them. Measured `dependencies.Inner` → `rewritten=true` vs `dependencies.default` → `rewritten=false`, with `compile_2020_12`'s `tracing::warn!` silently not firing. **Unlike rounds 1 and 2, NO v2 verdict flip is reproducible** — both names are `(Violates, Violates)` on the pinned library — so a behavioural assertion would have PASSED against the defective code, and the fence had to be STRUCTURAL (the `Cow` borrow/own decision plus the rewritten pointer). **✅ CLOSED A FOURTH TIME 2026-08-02 by `115-16`..`115-19`**, again as the owner's option **(a)** — recorded in `115-HUMAN-UAT.md` (Guy Ernest, 2026-08-02), who also chose FIX over accept-as-debt on the contract's stale equation head — and NOT as an override. `115-16` established the sixth entry by **DERIVATION** over the five meta-schema documents `jsonschema` 0.49.2 ships offline (object-typed keywords whose `additionalProperties` references the meta-schema itself; `$vocabulary` and `dependentRequired` excluded by that same criterion — the union is exactly six), rather than by patching the one case a reviewer found, and fenced it with an instrument carrying its OWN container literal, observed to fail on exactly the four `dependencies` pairs and no other container. `115-17` and `115-18` brought the two restated mirrors onto it — the fuzz copy was measured CRASHING on CORRECT behaviour while stale — and each kept its fence's REACHABILITY independent of the list under test, a discipline `115-17` learned by implementing its own plan literally and watching every negative control go green. `115-19` closed the RECURRENCE pattern that `115-REVIEW.md` WR-01 named and no prior round owned: three literal copies of two keyword lists, each rustdoc calling the mirror REQUIRED, with **no gate that they agree**. `tests/keyword_list_mirrors.rs` is that gate — featureless, so it runs inside `make quality-gate`; importing nothing from the crate, so it reaches the `fuzz/` copy the workspace `exclude` array hides from every other gate (`D-115-AB`); comparing all three as ORDERED sequences AND against the meta-schema-derived expectation, which is what catches the LOCKSTEP removal that today deletes coverage with zero test failures. `115-19` also rescoped the `output_schema_draft_pin` equation head, its `walk:` clause, both affected invariants and the three `binding.yaml` note heads to ONE stated scope over six keywords, with the residual named rather than hidden. Gate over the closed tree: `make quality-gate` exit **0** at **5060 passed / 0 failed / 81 ignored across 312 result lines** — after a FIRST run that exited 2 on a transient macOS keychain `ioErr -36` at a pre-existing native-roots `.expect`, discarded only after the identical binary passed standalone and booked as `D-115-AL` rather than quietly re-run; `pmat quality-gate --checks complexity` **0 violations**; SCHM-02/03 unregressed at **78/78**; a **3 614 479**-run `+nightly` campaign clean over 15 seeds with an EMPTY artifacts dir; no `Cargo.toml`/`Cargo.lock` in the closure diff and **0** new `pub fn`/`struct`/`enum` under `src/`. SCHM-01 re-booked in `.planning/REQUIREMENTS.md` AFTER those commands ran, all three prior records amended rather than deleted. **✅ SCORED AND CLOSED 2026-08-02.** `115-VERIFICATION.md` re-ran against the round-4 closure and returned **4/4 must-haves with NO BLOCKER** — the first of four passes without one. Both round-3 gaps independently re-confirmed closed (all three `SUBSCHEMA_MAP_KEYWORDS` copies byte-identical at six entries; the contract equation head now scoped to "any SCHEMA POSITION of s"). The round-4 review returned 1 critical + 6 warnings: **CR-01 was FIXED** (`71a44f40` — `tests/keyword_list_mirrors.rs` reads `fuzz/**` at runtime and was being PACKAGED while `fuzz/` is excluded, so a published-crate `cargo test` would have panicked; measured with `cargo package --list`, fixed beside the two neighbouring exclude entries that exist for the identical reason). The residual **WR-03** — array descent (`allOf`/`anyOf`/`oneOf`/`prefixItems`) is implemented at `output_validation.rs:265`/`:325` but fenced by NO test, property draw or seed, and absent from the contract's `SCHEMA POSITION` definition; **deleting both `Value::Array` arms passes the entire suite**, measured twice independently — did NOT reopen SCHM-01: unlike all three prior rounds the CODE is correct and unconditional, and an array position has no author-chosen name for a `DATA_ONLY_KEYWORDS` collision to hide behind, which is the exact shape that reopened this three times. Owner **Guy Ernest ratified that verdict with `approved` on 2026-08-02**, selecting defer-and-book; WR-03 and the remaining findings are booked as **`D-115-AM`**, residual and unowned, never silently absorbed. Final gate: `make test` **2700 passed / 0 failed**
- [x] **Phase 116: Auth Hardening SEPs** — RFC 9207 `iss` validation (strict v2 / lenient v1), DCR `application_type`, issuer-keyed credential storage + three clarifications — all source changes to the hand-rolled OAuth stack, no new crates (completed 2026-08-07)
- [x] **Phase 117: Agents, Tester & v1 Severability** — `pmcp-agent` (ToolInvoker + task polling) and `mcp-tester` exercise a v2 server end-to-end; v1-only machinery isolated behind a severable era-gated layer with a documented sunset policy; v2 path carries no session/SSE baggage (completed 2026-08-08)
- [x] **Phase 118: Conformance Against the Official Suite** — official `@modelcontextprotocol/conformance` (commit-pinned) in CI over real HTTP against a dual-version example; Phase-109 Rust harness gains v2 fixtures (v1 stays green, dev-dep-free build); deprecated caps verified functional under v2 (completed 2026-08-10)
- [x] **Phase 118.1: Close the Nine Conformance Gaps** — the nine structural SDK defects G-1..G-9 that Phase 118's measurement found: nested `EmbeddedResource` + `blob` + `annotations`; the `completion/complete` seam and v2 method retirement; the `_meta` classifier, `-32020`/`-32022` ordering and `supportedVersions`; the server-to-client back-channel over StreamableHTTP; and v1 capability plumbing. Ends in a re-measurement, a per-gap FIXED/REFUTED/DEFERRED disposition, and a gate widened to exactly what passes (completed 2026-08-12)
- [ ] **Phase 118.2: v1 Client SSE Transport + `notifications/message` Emitter** — the two residuals Phase 118.1 measured and could not close in scope, both signed off as OPEN sub-items of G-3 at the 118.1-13 D-10 gate: pmcp's own `StreamableHttpTransport` client cannot hold a live GET SSE stream (`collect_body_within_cap` whole-body read), so it cannot consume the v1 server-to-client channel 118.1 built; and no handler-facing emitter exists for `notifications/message`, the ONE remaining gap-attributable suite failure (`GAP_ATTRIBUTABLE_FAILURES = 1`)
- [x] **Phase 119: Documentation — Three Shapes + v2 Migration** — Agents & Teams docs in three shapes (carried from v2.4 Phase 111); v2 migration guide + dual-version story + sunset policy; runnable stateless-v2-server and v2-client/agent examples (completed 2026-08-19)

## Phase Details — v2.5 (MCP Spec 2026-07-28 v2 Support)

### Phase 112: Version Plumbing Spine

**Goal**: pmcp resolves a per-request protocol era once at transport ingress and threads it explicitly through dispatch, so one binary understands both 2025-11-25 and 2026-07-28 clients — with v2 strictly opt-in, no v1 behavior change, and the whole milestone kept additive (2.x minor). This is the keystone: nearly every other v2 behavior era-gates off it.
**Depends on**: Nothing (keystone; lands first and alone)
**Requirements**: VERS-01, VERS-02, VERS-03, VERS-04, VERS-05, VERS-06, VERS-07, VERS-08, VERS-09
**Success Criteria** (what must be TRUE):

  1. A v2-opt-in server resolves a `ProtocolContext` (era, negotiated version, clientInfo, clientCapabilities) once at transport ingress from per-request `_meta` (`io.modelcontextprotocol/protocolVersion`/`clientInfo`/`clientCapabilities`), a handler reads it via typed accessors on `RequestHandlerExtra`, and v2 results carry `serverInfo` (VERS-01, VERS-03)
  2. An existing v1 client negotiates exactly as before — `LATEST_PROTOCOL_VERSION` stays pinned to 2025-11-25 and 2026-07-28 is reached only through explicit opt-in (VERS-02)
  3. A v2 client calling `server/discover` receives a read-only projection of already-computed ServerCore capabilities, including the `extensions` capability map of reverse-DNS IDs (VERS-04, VERS-08)
  4. On the v2 HTTP path the required headers `Mcp-Method`/`Mcp-Name` (alongside `MCP-Protocol-Version`) are enforced inbound and emitted outbound (VERS-05)
  5. Every result carries the `resultType` envelope discriminator (`complete`/`input_required`/`task`), defaulting to `complete` when absent; W3C trace-context keys (`traceparent`/`tracestate`/`baggage`) in `_meta` are surfaced via typed accessors and propagated; and all error codes resolve from one centralized version-gated constant table with v2 values filled ONLY from the final 2026-07-28 schema.json and the frozen v1 `-32002` task-pending semantics unchanged (VERS-07, VERS-09, VERS-06)

**Plans**: 10 plans (8 shipped + 2 gap-closure)

Plans:
**Wave 1**

- [x] 112-01-PLAN.md — Version era classifier (2026-07-28 const, Era, protocol_era) + ProtocolContext/TraceContext types + semver gate

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 112-02-PLAN.md — RequestHandlerExtra protocol_context field + era/identity/trace accessors (native only — src/server/cancellation.rs; wasm RequestHandlerExtra is a zero-field stub, out of scope)
- [x] 112-03-PLAN.md — Centralized version-gated error-code table (standard + pmcp -320xx family; frozen -32002 verbatim; v2 values structurally OMITTED, zero-SATD) + error::ErrorCode's 11 consts DELEGATE to it (dominant 210-site surface) + server/discover via CRATE-PRIVATE internal dispatch (public ClientRequest/Request UNCHANGED — no downstream exhaustive-match break)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 112-04-PLAN.md — v2 opt-in accept-list builder + ONE shared resolve_protocol_context() enforcing the accept-list, resolved once at ingress & threaded (both native sites; native-only plumbing — resolver compiles on wasm32 with no wasm caller; malformed reserved _meta → typed error)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 112-05-PLAN.md — Era-gated dispatch: server/discover projection via internal dispatch (v1 -32601), pinned resultType envelope model (v2-only, object-only; native-only — v2 unreachable on wasm), serverInfo, v1 byte-identity golden fixtures
- [x] 112-06-PLAN.md — v2 HTTP header enforcement CONSUMING Plan 04's resolved era (no 2nd resolver): FULL header/_meta classification matrix incl. required MCP-Protocol-Version + Mcp-Method/Mcp-Name strict reject (D-05) + body cross-check (D-06) + outbound emission on success AND error

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 112-07-PLAN.md — Migrate ~40 error-code literal call sites (core/mod/task_dispatch/jsonrpc) onto the centralized error_codes:: table; frozen -32002/-32601 byte-identical
- [x] 112-08-PLAN.md — Migrate the streamable-HTTP transport's 25 error-code literals onto the centralized error_codes:: table; wire byte-identical, #[cfg(test)] oracle preserved

**Wave 6** *(gap closure — from 112-VERIFICATION.md; dispatch-wiring completeness)*

- [x] 112-09-PLAN.md — Gap B/C: generalize extract_request_meta_value to GetPrompt/ReadResource + thread protocol_context/request_meta into prompt & resource handlers at BOTH native sites; live HTTP prompts/get + resources/read v2 acceptance + v1 golden byte-identity (VERS-01/03/05/07/09)
- [x] 112-10-PLAN.md — Gap A: wire a live server/discover production caller on the HTTP POST path via parse_request_or_internal + shared capability projection (v2 → capabilities+extensions, v1 → -32601); remove stale #[allow(dead_code)] (VERS-04)

### Phase 113: Stateless HTTP + Multi-Round-Trip Elicitation

**Goal**: v2 HTTP requests run with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto pmcp's existing `stateless()` branch (not a transport fork); multi-round-trip elicitation works end-to-end; and the pmcp `Client` is the v2-speaking counterpart, folding the Phase-106 host handlers into the v2 flow. v1 session behavior is untouched.
**Depends on**: Phase 112 (ProtocolContext / era gate)
**Requirements**: HTTP-01, HTTP-02, HTTP-03, HTTP-04, HTTP-05, HTTP-06, HTTP-07, HTTP-08, HTTP-09, CLNT-01, CLNT-02, CLNT-05

> **HTTP-04 was split on 2026-07-26.** It had bundled ten obligations behind one checkbox and was
> the sole paragraph-length entry in `REQUIREMENTS.md` — every other requirement in that file is a
> single sentence. All seven gap-closure plans in this phase (113-14…113-20) targeted HTTP-04 and
> no other requirement, because a single checkbox covering ten obligations can never partially
> close: each review reopened the whole thing. The split is HTTP-04 (method removal + replacement),
> HTTP-06 (GET-stream transport removal), HTTP-07 (frame protocol), HTTP-08 (opt-in capability
> gating), HTTP-09 (bounded reads — **new, and NOT met**), and CLNT-05 (client half, moved to the
> CLNT section where the other client-side mirrors live). D-11 positioning and the instance-local
> `ListenRegistry` limitation were removed from the requirement entirely — neither has a pass/fail
> condition.
**Success Criteria** (what must be TRUE):

  1. A v2 HTTP request completes with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto the existing `stateless()` branch, while v1 session behavior is unchanged (HTTP-01)
  2. A handler returns `input_required` with `inputRequests` and an opaque `requestState` that is integrity-protected, principal-bound, and TTL'd; a client retry carrying `inputResponses` + the echoed `requestState` resumes the operation correctly (multi-round-trip elicitation end-to-end) (HTTP-02, HTTP-03)
  3. On the v2 path `resources/subscribe`/`unsubscribe` and the HTTP GET stream endpoint are **removed**, and v2 change notifications arrive over a `subscriptions/listen` long-lived stream (`toolsListChanged`/`promptsListChanged`/`resourcesListChanged`/`resourceSubscriptions` opt-ins, `subscriptionId` tagging, `notifications/subscriptions/acknowledged` first). The stream is **opt-in** — pmcp's stateless enterprise default advertises no subscription-delivered capability, and answering `subscriptions/listen` with method-not-found in that configuration is conformant; a tripwire test enforces that advertising any subscription capability requires serving the stream. The **client half** ships as `Client::subscriptions_listen`, returning a typed `SubscriptionStream`, with the retired `subscribe_resource`/`unsubscribe_resource` failing fast via a typed `retired_on_v2` error on v2. Per **D-11** polling over Tasks remains pmcp's RECOMMENDED enterprise mechanism, documented as a pmcp extension and not as a conformant substitute. **Constraint:** the `ListenRegistry` is instance-local, so advertising a subscription capability behind a non-sticky load balancer under-delivers; a build-time `tracing::warn!` names this but does not prevent it. (HTTP-04)
  4. SSE resumability (`Last-Event-ID`) is not offered on the v2 path, and a regression test proves response JSON-RPC ids are always derived from the live request — closing the id-replay / discovery-cache bug class (HTTP-05)
  5. The pmcp `Client`, selected explicitly per connection, speaks v2 (per-request `_meta`, `server/discover`, required headers, no `initialize`) and fulfills MRTR `input_required` results by producing `inputResponses`, with the Phase-106 host handlers (sampling/elicitation/roots) folded into the v2 flow (CLNT-01, CLNT-02)

**Plans**: 32 plans — 13 original (113-01…113-13) + 7 gap-closure round 1 (113-14…113-20) + 12 gap-closure round 2 (113-21…113-32, planned 2026-07-27; 113-29…113-32 supplemented after the RC spec-research pass landed Findings 11-14)

Plans:
**Wave 1**

- [x] 113-01-PLAN.md — Foundations: an ENFORCING three-state final-spec verdict (PUBLISHED-CONFIRMED/DRIFT/PENDING) that also re-pins the conformance-suite commit and records the contract-first/PDMT/PMAT environment, `ring`+`zeroize` promoted to explicit optional deps under `streamable-http` (blocking package-legitimacy checkpoint), and the three v2 transport error codes -32020/-32021/-32022 with locking tests
- [x] 113-02-PLAN.md — MRTR protocol-type layer: wire types plus the public `InputRequiredResult`/`MrtrOutcome` client-outcome types, the ONE MRTR-eligible-method + logical-name table, FAIL-LOUD params extract (`Result<_, MrtrParseError>`) and stale-key-clearing splice, five DoS bounds, AAD salient-param digest, `Mcp-Name` base64 sentinel codec, `ElicitRequestParams` mode-optional serde fix, and the shared `tests/common/v2.rs` harness

**Wave 2** *(blocked on Wave 1)*

- [x] 113-03-PLAN.md — `requestState` AEAD token (`src/server/request_state.rs`): SERVER-OWNED `Arc<RequestStateCodec>` built once at server build (no `OnceLock`), builder key/previous-keys/TTL + injectable clock, fail-the-build on a malformed configured key, mint/verify with principal‖method‖param-digest AAD, the D-15 verdict table with `Expired(Continuation)`, property tests and a `fuzzing`-feature fuzz target
- [x] 113-04-PLAN.md — HTTP-01 stateless era gate: one `sessions_active(state, era)` predicate routing all four session sites, GET/DELETE→405, RAW-level unknown-method→404, the -3202x→400 status mapper with structured `Reject { code, message, data }`, the locked `Mcp-Name` presence-always rule, sentinel-decoded cross-check, and `tests/v2_stateless_http.rs` against a STATEFUL default config

**Wave 3** *(blocked on Wave 2)*

- [x] 113-05-PLAN.md — CLNT-01 client v2 transport: the additive defaulted `Transport::set_negotiated_protocol_version` mode seam, validated `with_protocol_version` opt-in, per-request `_meta` with registry-derived capabilities and trace-context merge, no-handshake path, ERA-AWARE capability enforcement, `server/discover`, the three required outbound headers (empty `Mcp-Name` for name-less methods) with sentinel encoding, and session-id suppression
- [x] 113-06-PLAN.md — MRTR server ingress: MRTR params carried on `ProtocolContext`, extracted once from the raw v2 body with malformed input REJECTED at -32602, verified against `AuthContext.subject` (fail-closed for unauthenticated callers on auth-configured servers) + live params, D-15 routing with `Reelicit` = strip-and-RE-RUN the handler (round preserved on expiry), `RequestHandlerExtra` accessors, and the live verdict suite

**Wave 4** *(blocked on Wave 3)*

- [x] 113-07-PLAN.md — CLNT-02 client MRTR loop: typed `MrtrRoundLimitExceeded` and `InputRequiredUnfulfilled` without new `Error` variants, additive `*_mrtr` methods returning `MrtrOutcome`, preflight-before-invoke, the three-way fold through the FULL host pipeline (approval + result review), and the bounded gather→resend loop with a fresh id and stale-key-free params per round (mock transport only)
- [x] 113-08-PLAN.md — HTTP-05: `resumability_active` era gate turning `Last-Event-ID`/EventStore reads AND writes off on v2 (proven by a spy), plus the STRUCTURAL `envelope_for_live_request(payload, live_id)` constructor scoping the id invariant to DIRECT responses while v1 historical replay keeps its original ids
- [x] 113-09-PLAN.md — MRTR server egress: handler `_meta` signal → capability precheck → AEAD mint → `resultType:"input_required"` at both dispatch sites, unconditional `strip_mrtr_signal` on EVERY path (v1 included), SERVER-OWNED reserved fields (`resultType`/`serverInfo`/`requestState`/`inputRequests` overwritten or removed), the exhaustive eligible-method tripwire, submode-aware `-32021`, and the `serverInfo`→`result._meta` placement fix

**Wave 5** *(blocked on Wave 4)*

- [x] 113-10-PLAN.md — HTTP-04 server `subscriptions/listen`: subscription wire types locked from the spec checkpoint, the capability gate shared with the discover projection, the ack-first `(principal, RequestId)`-keyed SSE stream with a BOUNDED channel, RAII `ListenGuard` teardown and shared-envelope framing (opt-in, bounded per principal, documented instance-local), v2 retirement of `resources/subscribe`/`unsubscribe`, and the advertise-implies-serve tripwire
- [x] 113-11-PLAN.md — MRTR end to end: `113-CONFORMANCE-MANIFEST.md` generated from the PINNED conformance commit with a must-be-empty Unmapped section, a Rust mirror of every `sep-2322` scenario, a real-Client↔real-server multi-round exchange, and the runnable `examples/s47_v2_stateless_mrtr.rs` + `examples/s48_v2_mrtr_client.rs` pair

**Wave 6** *(blocked on Wave 5)*

- [x] 113-13-PLAN.md — HTTP-04 CLIENT half (added by the cross-AI review replan): `Client::subscriptions_listen` returning a typed `SubscriptionStream` that enforces acknowledgement-first and subscriptionId tagging, era-gating the retired `subscribe_resource`/`unsubscribe_resource` to a typed `retired_on_v2` error, and live proof a pmcp v2 client receives change notifications

**Wave 7** *(blocked on Wave 6)*

- [x] 113-12-PLAN.md — Phase gate: the feature/target build matrix under the absolute rustup cargo (no-default-features, wasm32, `fuzzing` unreachable from `full`, two verbatim dev-dep-free commands), `cargo semver-checks`/`cargo public-api` additivity, `make quality-gate` + PMAT complexity + per-file coverage + the 20k-run fuzz target, the contract-first environment record, and the EVIDENCE-GATED HTTP-04 reword + requirement flips + SEP-2243 gap record

**Gap-closure round 1** *(after `113-VERIFICATION.md` returned `gaps_found`)*

- [x] 113-14-PLAN.md — listen-registry collision safety: a duplicate LIVE `(principal, subscriptionId)` registration refused rather than evicting the incumbent, and every removal ownership-scoped by a per-entry generation
- [x] 113-15-PLAN.md — the SSE line-buffer bound moved INSIDE `SseParser` with a latching `overflowed()` flag, one enforcement point covering every present and future feeder; `connect_sse` guarded too
- [x] 113-16-PLAN.md — the bound-taking fuzz seam (`decode_listen_chunks_for_fuzz`) plus a measured 20 000-run campaign with branch coverage proven from the retained corpus
- [x] 113-17-PLAN.md — `SseParser::feed`'s bound made UNCONDITIONAL over retained-state + chunk (GAP-A), both whole-body transport sites routed through `feed_complete_body`, and `connect_sse`'s ceiling made configurable without a public config-struct change
- [x] 113-18-PLAN.md — GAP-B closed by CONTRACT: all three listen refusals become the retryable `RATE_LIMITED` at HTTP 200, the fresh-id reconnect contract pinned by a live tripwire, and WR-06's semaphore-leak race closed by `prune_after_rejection`
- [x] 113-20-PLAN.md — a STREAMING collected-body cap (`http_body_util::Limited`) at all three `StreamableHttpTransport` whole-body reads, discharging T-113-84; `Content-Length` an early-refusal optimisation, never the authority
- [x] 113-19-PLAN.md — round-1 phase gate: the fuzz seam gated off the public API (`cargo public-api` is blind to `doc(hidden)`, so the prior criterion passed vacuously) and the fuzz target's tautological latch invariant replaced by a per-chunk retention assertion proven falsifiable

**Gap-closure round 2** *(planned 2026-07-27, after the 2026-07-26 full-phase review; the three BLOCKERs it named were fixed in commit `5f045086` and are NOT re-planned here)*

**Wave 1**

- [x] 113-21-PLAN.md — HTTP-09 bounded-read source tripwire: runtime discovery of `src/shared/`, comment/literal-stripped scanning with line mapping, a structural `Limited`-in-statement rule for whole-body reads and a justified allowlist for peer-byte accumulations, with five recorded negative controls
- [x] 113-22-PLAN.md — HTTP-09 O(n) half: a FALSIFIABLE linear-time budget for `take_utf8_prefix` and `SseParser::feed` (the existing guard passes on the very quadratic shape it names), plus a retained-tail property test
- [x] 113-23-PLAN.md — D-113-N: the `subscriptions/listen` route fails closed on an auth-configured server instead of minting a private `anon#N` that makes the per-principal cap unreachable; plus the Finding-5 audit of pmcp's actual `subscriptionId` emission on all three frame classes
- [x] 113-24-PLAN.md — D-113-L: a server-side `MAX_MRTR_ROUNDS` ceiling enforced at the ingress verdict and at the mint, so the D-09 security counter stops being enforced solely by the client it exists to constrain
- [x] 113-25-PLAN.md — D-113-P: `requestState` key material zeroized on both builders and `resolve_codec_at_build` taking the key by reference, closing all three unscrubbed copies without breaking by-value builder chaining

**Wave 2** *(blocked on Wave 1)*

- [x] 113-26-PLAN.md — D-113-M: `write_canonical`'s depth-cap marker deleted and the canonicaliser made fallible, so two requests differing only below depth 64 can no longer share one AEAD AAD (replay-prevention clause 5c)

**Wave 3** *(blocked on Wave 2)*

- [x] 113-27-PLAN.md — D-113-O: `inputResponses` typed KIND-DIRECTED at server ingress from the sealed continuation's own record, replacing the best-effort untagged guess that silently reclassifies a wrong-shaped answer into an infinite re-elicitation

**Wave 4** *(blocked on Wave 3; added by the RC spec-research supplement — see `113-SPEC-RECHECK-ADDENDUM-2026-07-26.md` Findings 11-14)*

- [x] 113-29-PLAN.md — Finding 11: `basic/index.mdx` says implementations of this version MUST NOT emit `-32002`, but pmcp's two call sites (`server/core.rs:2616`, `task_dispatch.rs:605`) have never had v2-path reachability traced; traced by execution rather than inspection, and era-gated if reachable
- [x] 113-30-PLAN.md — Finding 13: BOTH false clauses retired from the `# D-11` rustdoc — the one the addendum quoted *and* "the only spec-conformant delivery shape for `listChanged`", false for the same reason one sentence later. The block now names the spec's real polling shape (`ttlMs`/`cacheScope`, SEP-2549, `server/utilities/caching`, blessed *instead of* `listChanged`), states that pmcp implements none of it (re-measured at execution time: zero hits in `src/`) and cross-references SCHM-03/Phase 115 — with D-11's conclusion unchanged and stated first, and the second clause replaced by a checkable claim about pmcp ("the only delivery shape pmcp CURRENTLY implements"). Two guards keep it out: an `include_str!` self-scan whose forbidden phrases are assembled at runtime from sub-40-char fragments (so it cannot contain its own needle, nor decay into `contains("")`) and flattened against comment markers (so a rustdoc line wrap cannot hide a reintroduction — proven by control B), plus a companion requiring the replacement to keep naming `ttlMs`/`cacheScope`/`SEP-2549`/`SCHM-03`. RED before the edit; three negative controls run and reverted. Finding 14a recorded as **D-113-S** (not the plan's D-113-Q — A..R are all in use), blocked on missing information and not difficulty: Phase-112 D-05 requires `Mcp-Name` on every v2 request and stdio has no headers. Finding 14b left to 113-31. `.planning/REQUIREMENTS.md` untouched, no checkbox flipped
- [x] 113-31-PLAN.md — Finding 14b CLOSED: the resources half of HTTP-08's four capability opt-ins had ZERO end-to-end wire tests (`grep -rn "resourceSubscriptions|resources/updated" tests/ examples/` returned nothing but binary `.wasm` matches); it is now **21 real hits** across four live-socket tests. `covers`'s `ResourceUpdated` arm is proven EXACT-STRING selective with the unsubscribed URI fired FIRST (so "filtered" is distinguishable from "slow"); the two resources opt-ins are proven independent by asserting the capability cross-product in BOTH directions, with the omitted agreed field checked as key **ABSENCE** rather than falsity (matching `skip_serializing_if`); and the `MAX_AGREED_RESOURCE_SUBSCRIPTIONS` truncation is observed in a real acknowledgement, driven from the IMPORTED constant with probe URIs chosen by INDEX. **Five negative controls, each failing exactly ONE of the four tests** — the orthogonality is the evidence they are not restatements of one another; the fifth exists because removing `.take(..)` short-circuits the truncation test before its delivery assertion, leaving half of it unproven. Zero production bytes changed, no production defect found. New deferral **D-113-T** (pre-existing nextest `LEAK` on 4 older tests in the same file — measured at 4/12 runs, zero on the new four across 16; recorded, not swept). `.planning/REQUIREMENTS.md` untouched, no checkbox flipped
- [x] 113-32-PLAN.md — Finding 12: HTTP-08's advertise-implies-serve predicate has NO spec sentence behind it — it lives in the conformance repo, which the schema-only gate cannot see; adds a second gate arm pinning a conformance sha verbatim from upstream

**Wave 5** *(blocked on Wave 4)*

- [x] 113-28-PLAN.md — the publication-gate THIRD-OUTCOME decision (checkpoint): the binding re-verification procedure has no branch for `schema/2026-07-28` still not existing on the date; assembles the evidence brief and records the maintainer's policy without flipping any checkbox. Amended with Findings 7/8/10 — the RC is a strict ancestor of our pin 236 commits behind, the three constants under exception were renumbered *after* the RC lock, and the gate's trigger is restated as a condition ("a versioned schema directory exists") rather than the date

**Phase-gate outcome (plan 12):** 16/16 build-matrix rows exit 0; `cargo semver-checks` 223/223 pass with no update required and `cargo public-api` shows **zero** removed public items, so the milestone is provably still additive; `make quality-gate` exits 0; all seven new/changed files clear the 80% coverage target; the 20k-run fuzz campaign passed with zero crash artifacts. **The phase is NOT closed as complete** — `113-SPEC-RECHECK.md`'s `## Verdict` is still `PENDING` (re-verified 2026-07-26: no `schema/2026-07-28` upstream), so HTTP-01..05 and CLNT-01..02 are marked `[~]` implemented-pending-final-schema rather than complete. See `113-12-SUMMARY.md`.

### Phase 113.1: Merge Unblock (INSERTED)

**Goal:** Clear the three things that keep the `fix/mcp-publisher-oidc-audience` branch from merging, so Phases 114-119 can start. Two of them (D-113-R, D-113-Q) are what hold **HTTP-09** at `[ ]` on the merits; the third (D-113-U) is the org-required CI `gate` check itself.
**Requirements**: HTTP-09
**Depends on:** Phase 113
**Plans:** 6 plans in 3 waves

**Scope** (the three blockers, verbatim from the milestone phase list):

  1. **D-113-U — PR-blocking PMAT complexity.** `handle_post_fast_path` (cog 30) and `handle_post_with_middleware` (cog 31) against the hard ceiling of 25, reaching CI through the org-required `gate` check. Both were 22/21 on `main` and were pushed up by earlier commits on this branch. The identified fix is extracting the v2 header gate — copy-pasted between the two handlers — into one `resolve_v2_gate` helper (`FastPathDispatch`/`MiddlewareDispatch` are also field-for-field identical).
  2. **D-113-R — quadratic scan over peer-chosen input.** `drain_complete_lines`'s per-CALL cost: `consumed` restarts at 0 each `feed()`, so a peer sending 1-byte chunks gets one full-buffer rescan per byte. Distinct from the per-line drain quadratic already fixed in `0493d9fb`. Violates HTTP-09's explicit O(n) clause.
  3. **D-113-Q — unbounded peer read.** `src/shared/sse_optimized.rs:266`'s unbounded `reqwest::Response::text()`, currently allowlisted `NOT BOUNDED` in the Phase-113 bounded-read tripwire.

**Also in scope (record, not necessarily fix):** the 18 unbounded `reqwest` reads in `src/client/oauth.rs`, `src/client/auth.rs` and the two auth providers — same defect class, outside the tripwire's scope fence, semi-trusted IdP rather than arbitrary peer.

**Success Criteria** (what must be TRUE):

  1. `pmat quality-gate --fail-on-violation --checks complexity` passes with no `src/` function over cog 25, and the org-required `gate` status check is green on the branch (D-113-U)
  2. No scan over peer-chosen input on the v2 transport path is worse than O(n), proven by a falsifiable linear-time budget that fails on the current `drain_complete_lines` shape (D-113-R)
  3. The bounded-read tripwire's allowlist no longer carries `sse_optimized.rs:266` as `NOT BOUNDED` (D-113-Q)
  4. **HTTP-09 flips from `[ ]` to met on the merits** in `.planning/REQUIREMENTS.md` — not by narrowing the requirement
  5. The 18 auth-path unbounded reads are recorded as a named deferral with an owning phase

**Outcome (2026-07-27).** The Goal, Scope and Success Criteria above are the phase's CHARTER, quoted
as written; they are not current-state claims. What shipped:

  - SC-1 **split**: **SC-1a discharged** — `pmat quality-gate --fail-on-violation --checks complexity`
    passes locally with zero violations, both handlers at cognitive **15** (from 30 / 31).
    **SC-1b NOT discharged** — the org-required `gate` needs a human push (D-20), and two
    PRE-EXISTING CI failures stand in front of it: `make doc-check`'s 26 rustdoc errors
    (**D-113-W**, proven present at HEAD before this phase) and the Purity Gate's tooling drift.

  - SC-2 **met** — two falsifiable O(n) guards, each RED before the fix and RED again under a
    post-fix negative control.

  - SC-3 **met** — `sse_optimized.rs`'s read is bounded and `WHOLE_BODY_ALLOWLIST` is EMPTY, so the
    `NOT BOUNDED` entry named in Scope item 3 no longer exists.

  - SC-4 **met** — HTTP-09 is `[x]` with its requirement text byte-unchanged.
  - SC-5 **met**, and the charter's figure corrected: the auth population is **31** reviewed-unbounded
    reads across **four** files, not 18 across three. The tripwire's needles are single-line
    substrings and rustfmt splits these call chains, so the original count undercounted. Recorded as
    **D-113-V**, owner Phase 116.

  Closure record: `.planning/phases/113.1-merge-unblock/113.1-06-SUMMARY.md`.

Plans:

*Wave 1 (parallel — disjoint files):*

- [x] 113.1-01-PLAN.md — D-113-U part 1: extract the copy-pasted v2 header gate into the `resolve_v2_gate` / `resolve_v2_gate_with_error_hook` sibling pair and the fast path's inline ingress dispatch into `dispatch_message_fast` (D-06/D-09), with three recorded negative controls (D-11). Lands at a measured 26/28 — the gate is closed by 113.1-05
- [x] 113.1-02-PLAN.md — D-113-R: four falsifiable guards written and recorded RED, then the scan-window cursor plus the `debug_assert` removal as ONE atomic change (D-12+D-15) — all in a single green commit, with a post-fix negative control proving the guards still falsify (D-13/D-16), followed by a MANDATORY fuzz campaign. Shipped: pre-fix RED 6.81 s / 15.06x, committed 63.6 ms / 4.39x, post-fix control RED 4.36 s / 14.85x
- [x] 113.1-03-PLAN.md — D-113-Q: bound `connect_sse` with a `reqwest::chunk()` running total against the crate's 16 MiB SSE ceiling (D-02/D-03), drive `WHOLE_BODY_ALLOWLIST` to 0 (D-05), add the accumulation entry the fix requires, and deprecate `OptimizedSseTransport` toward `StreamableHttpTransport` (D-01/D-04)
- [x] 113.1-04-PLAN.md — records only: the auth-surface unbounded reads enumerated (raw matches and reviewed-unbounded subset) and assigned to Phase 116 without widening the tripwire fence (D-18); the pre-existing `D-113-J` PMAT-recipe entry AMENDED IN PLACE with an owner, a status and a second measured trap rather than duplicated; and arm 2 of the rolled-forward re-verification executed against upstream HEAD `5cc567c3` (D-19). Shipped: the auth population re-measured at **31**, not 18 — the roadmap figure was a raw line-grep count; the tripwire's scanner would find all of them and only its SCOPE FENCE keeps these files unreported; arm 2 returned **NO DRIFT**, so Branch A was taken and no Phase-118 item was needed

*Wave 2 (blocked on 113.1-01 — same file):*

- [x] 113.1-05-PLAN.md — D-113-U part 2: the read/classify preamble and legacy-version guard extractions RESEARCH measured as necessary, landing BOTH handlers at cognitive <= 20 (D-10) and turning the PR-blocking PMAT gate green. Shipped: **15 / 15**, beating RESEARCH's 16/16 target row

*Wave 3 (blocked on 113.1-02/03/04/05):*

- [x] 113.1-06-PLAN.md — closure: HTTP-09 flipped to `[x]` on the merits at all three sites (D-17), the falsified ROADMAP warning-block claims corrected, D-113-Q/R/U marked RESOLVED, and the full phase gate run including the PMAT invocation `make quality-gate` does not cover (D-20)

### Phase 114: Tasks Extension Migration

**Goal**: Tasks become a v2 extension — a wire-API reshape behind the proven `serde_json::Value` `TaskRouter` boundary, not a storage rewrite — while v1 Tasks stay fully functional, all backends survive unchanged, and stateless v2 owner-binding fails closed (the critical no-session cross-caller-leak guard).
**Depends on**: Phase 112 (era gate); Phase 113 (the stateless per-request-identity pattern owner-binding reuses)
**Requirements**: TASK-01, TASK-02, TASK-03, TASK-04, TASK-05, TASK-06
**Success Criteria** (what must be TRUE):

  1. Tasks are negotiated on v2 via the extensions map (`io.modelcontextprotocol/tasks`) while v1 `experimental.tasks` negotiation continues to work (TASK-01)
  2. A client feeds input into a running task via `tasks/update`; v2 task-augmented results use `resultType:"task"` with `CreateTaskResult{taskId,status,ttlMs,pollIntervalMs}`, and the v1 5-state machine maps deterministically to the v2 status enum (`working|input_required|completed|failed|cancelled`) (TASK-02, TASK-04)
  3. `tasks/list` (and blocking `tasks/result` semantics per the final spec) are era-gated off on v2 while remaining fully functional for v1 consumers (TASK-03)
  4. On v2, task owner binding requires OAuth `sub` or a stable per-request identity and fails closed when absent (no session-id fallback); a security test proves no cross-caller task visibility (TASK-05)
  5. The `TaskStore` trait, state machine, and DynamoDB/Redis/in-memory backends are unchanged — the migration is a wire-API reshape behind the `TaskRouter` boundary, verified by the v1 storage/tasks test suite staying green (TASK-06)

**Plans**: 20 plans (12 waves)

Plans:

**Wave 1** *(no dependencies — parallel)*

- [x] 114-01-PLAN.md — Vendor the ext-tasks draft schema at a pinned commit (PROVENANCE + SHA256 tripwire) + the `114-SPEC-RECHECK.md` hold record with the both-repos condition (DQ6)
- [x] 114-02-PLAN.md — v1 `tasks/*` golden byte fixtures captured PRE-reshape (D-14 item 2; none existed) + shared tasks test harness (`OptionalBearer`, tasks-backed spawn, client-declaration body builder)
- [x] 114-03-PLAN.md — `ClientCapabilities.extensions` field (F6 gap) + `TASKS_EXTENSION_KEY` + typed `TasksExtensionCapability` serializing as `{}` + five serde locks
- [x] 114-04-PLAN.md — Additive `TaskStore` input-delivery + owner-scoped `task_input_snapshot` + `record_input_requests` + error persistence + `supports_inputs()` and `TaskRouter::handle_tasks_update` seams (D-12) + in-crate `InMemoryTaskStore` impls (D-13 site 3)
- [x] 114-20-PLAN.md — **Contract-first owner decision** (blocking checkpoint): measure the absent `../provable-contracts/` dependency and settle author-vs-waive BEFORE implementation, replacing 114-18's self-granted exemption

**Wave 2** *(parallel)*

- [x] 114-05-PLAN.md — Server extension advertisement via the shared endpoint-backed rule (D-01) + era-projected capabilities: v2 discover shows the entry and drops the v1 tasks keys, v1 `initialize` byte-identical (D-02/D-03)
- [x] 114-06-PLAN.md — Client half: per-request extension declaration, era-aware `assert_capability` reading the extensions map (D-04), `Mcp-Name` = `params.taskId` via a SEPARATE table keeping `tasks/update` MRTR-ineligible (DQ4)
- [x] 114-07-PLAN.md — `pmcp-tasks` input delivery in `GenericTaskStore<B>` (one CAS via `put_if_version`) + memory delegation (D-13 site 2, F12) + router override + pre-114 record byte fixture

**Wave 3**

- [x] 114-08-PLAN.md — `tasks/list` + `tasks/result` era-gated off on v2 with two distinct truthful `-32601` messages; frozen `-32002` untouched; `is_v1_task_era` rustdoc corrected (TASK-03)

**Wave 4**

- [x] 114-09-PLAN.md — v2 owner binding fails closed on Phase 113's three-row identity table (no session-id, no `client_id`); ordered refusals `-32021` then `-32003` at HTTP 200 before the params parse (DQ3); v1 `"local"` frozen + migration warn

**Wave 5**

- [x] 114-10-PLAN.md — Reserved-field registry fix: explicit ownership replaces the disposition-derived flag so a v2 `tasks/get` keeps its required top-level `inputRequests` (DQ2, highest-severity finding). The `ResponseDisposition::Task` dead-code allow is deliberately NOT removed here — see 114-12

**Wave 6**

- [x] 114-11-PLAN.md — v2 wire shapes: flat `CreateTaskResult`/`GetTaskResult` with `ttlMs`/`pollIntervalMs`, status-conditional `result`/`error`/`inputRequests`, empty acks, `NotFound` → `-32602` without an oracle; v1 shapes untouched (TASK-04)

**Wave 7**

- [x] 114-12-PLAN.md — Server-directed v2 create trigger: the client's per-request declaration replaces v1's `task` field (DQ1), enforced in ONE expression reached from both dispatch sites; the create→pause loop records handler-declared `inputRequests` against the STORE-minted id; `ResponseDisposition::Task` promoted to live code here (first production constructor); end-to-end over a real `tools/call`

**Wave 8**

- [x] 114-13-PLAN.md — `tasks/update` routing via `InternalClientRequest` (no public-enum variant — `ClientRequest` is NOT `#[non_exhaustive]`) + three replacement guards for the lost MRTR compile tripwire (Pitfall 4)

**Wave 9**

- [x] 114-14-PLAN.md — `tasks/update` delivery over a RAW map boundary: four input-response MRTR bounds FIRST (before the untagged decoder can run), kind-directed `decode_for` against kinds from `task_input_snapshot` (D-17 / the D-113-O class), atomic partial-vs-complete transition, empty ack + property test + fuzz target

**Wave 10** *(parallel — disjoint files: security tests, tripwire tests, client)*

- [x] 114-15-PLAN.md — TASK-05 live-socket two-principal cross-caller matrix over `tasks/get`/`update`/`cancel` with measured indistinguishability and per-method negative controls (D-09)
- [x] 114-16-PLAN.md — Source tripwires: every tasks route carries a named era guard, no v2 `NotFound` → `-32603`, status-string set-equality against the vendored schema, per-value provenance
- [x] 114-19-PLAN.md — **The v2 client half** (D-04/D-05's locked dual-surface steer): era-aware decoding of the flat create/get shapes and empty acks driven by `resultType`, `tasks_update()`, and a poll helper that reads the terminal result inline from `tasks/get` instead of the v2-retired `tasks/result`

**Wave 11**

- [x] 114-17-PLAN.md — The paired runnable example `s50_v2_tasks_server` / `s51_v2_tasks_agent` (autonomous agent poll loop, D-05; `s49` was already taken twice). Examples-only — all client work moved to 114-19

**Wave 12**

- [x] 114-18-PLAN.md — Whole-phase gate (quality-gate, semver + pmat asserted as deltas against a measured phase-base manifest, feature matrix, wasm), stale-doc sweep, TASK-01..06 booked `[~]` under the D-18 hold, deferred-items ledger + sign-off checkpoint; cites 114-20's contract decision and blocks on any 114-15 security defect

### Phase 115: JSON Schema 2020-12 + Structured Output + Caching Hints

**Goal**: Schema validation moves to an explicitly-pinned Draft 2020-12, v2 `structuredContent` accepts any JSON value (relaxing the 2.15 object-only bridge), and the list/read results carry additive caching hints — all wasm-clean and independent enough to parallelize with the HTTP/Tasks track.
**Depends on**: Phase 112 (era gate for validation strictness; parallelizable with Phases 113/114)
**Requirements**: SCHM-01, SCHM-02, SCHM-03
**Success Criteria** (what must be TRUE):

  1. Schema validation runs Draft 2020-12 explicitly pinned (jsonschema 0.48, no `$schema` auto-detect), staying wasm-clean and SEP-2106-compliant with no external `$ref` dereference (SCHM-01)
  2. On v2, `structuredContent` accepts any JSON value (scalar/array/null/object) while v1-negotiated tools keep the existing object-shaped behavior — proven against the 2.15 structured-output bridge (SCHM-02)
  3. The five list/read results carry additive `ttlMs`/`cacheScope` caching hints (SCHM-03)

> **Planning deviation (recorded 2026-07-31, `/gsd:plan-phase 115`):** criterion 3 says *five*; the
> plan set delivers **six**. The published core schema vendored at pinned commit
> `271ecc9accafdd9b83a3c869fa67c22953b2af80` has `DiscoverResult extends CacheableResult` alongside
> the five named results, and pmcp's `ServerDiscoverResult` is already routed through the same
> `inject_v2_result_envelope` chokepoint — so including `server/discover` is cheaper than excluding
> it, and excluding it would ship a knowingly non-conformant FIRST call for every v2 client.
> Criterion 1 says `jsonschema 0.48`; the plan set pins **0.49** (0.48.0-0.48.2 carry packaging
> defects fixed by 0.48.3-0.48.5; 0.49 is additive-only). Both deviations are booked inside the
> requirement records by `115-10`.
>
> **Replan deviation (recorded 2026-08-01, `/gsd:plan-phase 115 --reviews`):** a cross-AI review
> (`115-REVIEWS.md`) found seven blocking defects, all in the VERIFICATION design rather than the
> architecture. The plan set grew from ten to **eleven** with the addition of `115-11` (wave 1,
> contract-first). Contracts live IN-REPO at `contracts/`, not at the `../provable-contracts/` path
> CLAUDE.md names — that directory does not exist in this checkout — and `115-11` records the
> deviation. `115-10` books all three.
>
> **Execution deviation (recorded 2026-08-01, `115-10` Task 3, AFTER the owner sign-off).** Both
> notes above were re-verified against what actually landed. The five-vs-six and 0.48-vs-0.49
> readings hold as written, and the contract-location deviation is present in the replan note above
> and is confirmed shipped: the phase's contracts live in-repo at `contracts/`
> (`mcp-protocol-sdk-v1.yaml` + `binding.yaml`), NOT at the `../provable-contracts/contracts/<crate>/`
> path CLAUDE.md names, because that directory does not exist in this checkout. **CLAUDE.md was
> deliberately NOT edited** — rewriting a project-wide standing instruction is not a phase executor's
> call; the deviation is recorded inside the requirement bookings and in the ledger instead. Four
> further divergences the phase acquired during execution, none of which change the phase's shape:
> (1) the contract set is **fourteen** bindings, not the thirteen the replan planned — `115-10` added
> `compile_for_era`, which had none — and three recorded signatures were corrected to the shipped
> text (all three were the same harmless kind: an elided path the source writes in full, same types);
> (2) an exact `=0.49.2` version pin was **DECLINED** for a published library crate on semver
> grounds, and `Cargo.lock` is gitignored, so the bump has no reviewable lockfile diff;
> (3) `ResourceHandler` declares only `read` and `list` — there is **no templates method** — so only
> **two** of the six cacheable results are handler-settable and `resources/templates/list` can only
> ever carry SDK defaults; three copies of production rustdoc claiming otherwise were corrected;
> (4) `make test-feature-flags` exits 2 and the acceptance criterion demanding exit 0 was
> **unsatisfiable as written** — the target was already red at the phase base with a byte-identical
> 62-error per-file distribution, so **Phase 115's delta is ZERO**. It is carried as `D-114-U`, and
> the gate was neither weakened nor worked around. Full ledger: 36 items, every one owned or
> explicitly **unowned**, at
> `.planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md`.

**Plans**: 19 plans (11 shipped + 2 gap-closure + 2 gap-closure round 2 + 4 gap-closure round 3)

Plans:

**Wave 1**

- [x] 115-01-PLAN.md — Vendor the published 2026-07-28 core schema at a pinned commit with `PROVENANCE.md`, generalize the provenance tripwire to every tree under `schema/vendored/`, and re-derive the `CacheableResult` contract from the pinned artifact (D-14)
- [x] 115-02-PLAN.md — Pre-change raw-byte golden fixtures for the five v1 list/read responses, with a `ttlMs`/`cacheScope` leak guard proven to fire (D-13; MUST land before any field addition)
- [x] 115-11-PLAN.md — Contract-first (CLAUDE.md): three provable-contract equations for SCHM-01/02/03 in the IN-REPO `contracts/` tree, thirteen bindings landed as `status: planned`, and `tests/phase115_contract_bindings.rs` — the ghost-binding resolver `make comply` never had for `contracts/binding.yaml`

**Wave 2**

- [x] 115-03-PLAN.md — SCHM-01: `jsonschema` 0.49 across all three manifests, Draft 2020-12 pinned on v2 via normalize-then-compile (the naive pin is a measured silent validation BYPASS), `Era: Hash`, an era-keyed validator cache, and a draft-07 fence
- [x] 115-04-PLAN.md — SCHM-02: `CallToolResult::structured_value` sibling constructor, the era rustdoc, and scalar/array/null `structuredContent` coverage across both dispatchers (there is no object-only guard in pmcp to remove — measured)

**Wave 3**

- [x] 115-05-PLAN.md — SCHM-03 types: the closed `CacheScope` enum AND the **cfg-free** `Cacheable` + `project_caching_hints` projector in a new `src/types/caching.rs` (cfg-free so the wasm32-only dispatcher can reach it), `Option`-typed hint slots on all six `CacheableResult` types with builders on the three handler-reachable ones, 26 struct-literal sites restored, and serde locks derived from the vendored schema

**Wave 4**

- [x] 115-06-PLAN.md — SCHM-03 projection: a `Cacheable` claim captured before the request is moved, hints ensured on v2 and STRIPPED on v1 at the one shared chokepoint (D-12), wired into ALL THREE dispatchers including `wasm_server.rs` — closing a D-11 v1 leak the review found — with the post-projection response-middleware limitation measured and documented

**Wave 5**

- [x] 115-07-PLAN.md — SCHM-03 on the wire: six methods, two eras, both native dispatchers with an in-band `resultType` era witness, the v1 strip proven against a handler that genuinely opted in, and the measured bound that four of the six methods cannot reach v2 in-process asserted at a named test
- [x] 115-08-PLAN.md — Tripwires: SEP-2106 fenced against cargo's DECLARED and RESOLVED dependency graphs via `cargo metadata` (catching renamed/table-style/unification cases a text scan misses), the D-12 single-projection fence, the wasm call-site fence (the only gate that catches its removal), and the projection/middleware ordering fence
- [x] 115-09-PLAN.md — ALWAYS requirements: a `fuzzing`-gated three-state `SchemaVerdict` seam on the UNCACHED compile path, `fuzz_schema_draft_pin` with three TRUE invariants (the pre-review monotonicity invariant was FALSE) and a committed seed corpus, property tests, and `examples/s52_v2_caching_hints.rs` — all verified by direct commands because `make test-fuzz`/`test-property`/`test-examples` are fail-open

**Wave 6**

- [x] 115-10-PLAN.md — Stale-doc sweep + deferred-items ledger FIRST, then the whole-phase gate measured as deltas against a phase base (including the `wasm,validation` build `make wasm-build` never runs), SCHM-01/02/03 booked `[x]` on published evidence (D-15 — no inherited hold), and an owner sign-off after which — and only after which — the ROADMAP/STATE completion markers are applied. **The sign-off was returned UNANSWERED rather than self-approved and was APPROVED by Guy Ernest (owner) on 2026-08-01, with all three refused-to-self-approve items accepted (the 0.49/six-results/in-repo-contracts deviations, the ledger's unowned items, and `[x]` over `[~]`) and no corrections requested.** These markers were written after that answer

**Gap closure** *(planned 2026-08-01 from `115-VERIFICATION.md`, status `gaps_found`, 3/4 must-haves — SCHM-01 only; SCHM-02 and SCHM-03 re-measured as VERIFIED and untouched)*

- [x] 115-12-PLAN.md — CR-01: `normalize_schema_dialect` rewrote only the document ROOT `$schema`, so a legacy dialect declaration on an embedded schema resource (a subschema carrying `$id`) survived the v2 pin and resolved an EMPTY vocabulary set there — reproduced twice as `root-draft07 + embedded (v1,v2) = (Violates, Conforms)`, i.e. **v2 validating weaker than v1**. Normalization becomes recursive behind the unchanged `Cow`-returning signature, rewriting every STRING-valued `$schema` at any depth while skipping `const`/`enum`/`default`/`examples` payloads and `properties` entries named `$schema` (both are DATA, not dialect declarations — the fix sketch in the review would have corrupted them). Plus the gate-visible behavioural fence the three excluded layers could not host, the `$id`-bearing case in `normalization_cases()`, the purity postcondition, and the corrected rustdoc / contract invariants / research bullet that all asserted the false "the pin wins UNCONDITIONALLY" property
- [x] 115-13-PLAN.md — The generators that structurally could not reach the shape: `arb_schema_document()` gains `$id`+`$schema` embedded resources, the fuzz target gains a TOTAL invariant 5 (no legacy dialect survives normalization, implemented independently of the crate's own detector) plus `$defs`/`$id`/`$ref`-with-no-siblings in the neutrality allowlist — the nested-`$schema` exclusion deliberately STAYS, because after the fix v2 is legitimately stricter than v1 there and invariant 3 is an equality. Two committed seeds, a time-boxed `+nightly` campaign, `make quality-gate` + the PR-blocking `pmat --checks complexity`, and SCHM-01 re-booked on post-fix measured evidence as option (a) of the verification report's human-verification item. **Shipped:** the widened generator emitted the embedded shape in **100 of 256** cases and both new fences were OBSERVED to fail against a reverted root-only normalizer (property test: dialect-purity message; seed `12_embedded_legacy_resource`: exit 77) — an unfired fence is not evidence. 13 committed seeds, `-runs=0` replay clean, **3 951 202**-run campaign clean with an EMPTY artifacts dir. `make quality-gate` exit **0** (5052 passed / 0 failed / 81 ignored across 309 result lines) — the FIRST run failed on a `clippy::similar_names` error `115-12` introduced that a bare `cargo clippy -D warnings` cannot see, fixed by renaming (`cab8937a`); `pmat --checks complexity` **0 violations**; SCHM-02/03 unregressed at **78/78**. Also filed `D-115-AB`: `fuzz` is in the workspace `exclude` array, so the gate formats, lints, builds and runs NOTHING under `fuzz/`

**Gap closure — round 2** *(planned 2026-08-02 from the re-verified `115-VERIFICATION.md`, still `gaps_found`, 3/4 must-haves — SCHM-01 only; SCHM-02/03 re-measured VERIFIED at 78/78 and untouched)*

- [x] 115-14-PLAN.md — `115-12`'s recursion closed the root-only bypass but shipped a **position-blind** rule: `DATA_ONLY_KEYWORDS` is tested against every object key uniformly, without distinguishing a key in KEYWORD position from a key in NAME position. Since `$defs`/`properties`/`patternProperties`/`definitions`/`dependentSchemas` map AUTHOR-CHOSEN names to subschemas, a `$defs` entry literally named `default` is visited by neither walker — its legacy `$schema` survives the v2 pin and resolves an empty vocabulary set, the identical vacuous-validator bypass through a different document shape. Independently reproduced by the verifier: `$defs.Inner` → `(Conforms, Violates)` vs `$defs.default` → `(Conforms, Conforms)`, `rewritten=false`. Lands `SUBSCHEMA_MAP_KEYWORDS` position-aware traversal in BOTH `first_legacy_dialect` and `pin_dialect_in_place` (plus the non-object fallback CR-01's sketch omits), fences the colliding name in `normalization_cases()` **observed to fail first**, and corrects the two 🛑 false claims: the rustdoc asserting the pin wins "UNCONDITIONALLY across the whole DOCUMENT" (`:25-34`, `:199-222`) and the `output_schema_draft_pin` postcondition added by `115-12` Task 3 (`contracts/mcp-protocol-sdk-v1.yaml:284-292`). WR-03 (fragment-suffixed 2020-12 URI false-positive) **excluded with reason** as `D-115-AC` — the correct fix shape depends on an unmeasured `jsonschema` 0.49.2 resolution behaviour
- [x] 115-15-PLAN.md — The structural half: all three defensive layers `115-12`/`115-13` built **restate the same rule** as the code under test, so none of them can see a rule defect — which is why this is round 3 on one requirement. Propagates the position rule to both restated copies, then adds a fence whose invariant is *derived* rather than restated: **rename invariance** (2020-12 core/applicator — subschema-map keys are semantically inert author-chosen names, so `normalize(entry)` must not depend on the name it is filed under), landed as a proptest and fuzz invariant 6, consulting no keyword list and firing on the shipped defect. Parameterizes the hard-coded `"Inner"` (WR-06) over a colliding-name set with a measured coverage floor, commits seed `14_defs_named_default` observed to trip an invariant pre-fix, runs the CI-equivalent `make quality-gate` + PR-blocking `pmat --checks complexity`, and **only then** corrects SCHM-01's premature `[x]` booking in `.planning/REQUIREMENTS.md` — the booking task is gated on measured exit codes by construction, since `D-115-G` (booking ahead of evidence) has now recurred twice on this exact requirement

**Gap closure — round 3** *(planned 2026-08-02 from `115-HUMAN-UAT.md`, status `diagnosed` — the owner (Guy Ernest) answered BOTH open items from the `human_needed` verification with **fix, not defer**: option (a) on Gap 1 and the doc fix on Gap 2. SCHM-01 only; SCHM-02/03 re-measured VERIFIED at 78/78 and untouched. NOTE ON NUMBERING: this heading counts PLAN rounds (12/13, 14/15, 16-19) while `115-VERIFICATION.md` counts VERIFICATION passes — "round 3" here is the **fourth** time SCHM-01 has been booked closed, and the two schemes must never be silently reconciled.)*

- [x] 115-16-PLAN.md — Gap 1, code half. `SUBSCHEMA_MAP_KEYWORDS` was a five-entry allow-list omitting `dependencies` — draft-04..2019-09's own map-from-instance-property-NAME-to-subschema keyword, which this module's own test comment records (`D-115-03-C`) as still honoured by `jsonschema` 0.49.2 under the pin. Measured `dependencies.Inner` → `rewritten=true` vs `dependencies.default` → `rewritten=false`, with `compile_2020_12`'s `tracing::warn!` — the only D-02 diagnostic — silently not firing. **Unlike rounds 1 and 2, NO v2 verdict flip is reproducible** (both names measure `(Violates, Violates)`), so the fence is STRUCTURAL — the `Cow` borrow/own decision plus the rewritten pointer, the technique `115-14` already used for its `properties`-half — because a behavioural assertion would pass against the defective code. Bounds the fix by ENUMERATION over the pinned meta-schemas rather than by patching the reviewer's one case, carries its own container literal so it cannot be silenced by the omission it exists to catch, covers `patternProperties`/`dependentSchemas` (WR-02, in the list since 115-14 and exercised by nothing), publishes both lists through the `fuzzing` seam, fences their disjointness (WR-05 half), and corrects the falsified "deliberately a SUPERSET" rustdoc to the scope the code actually has — including the residual it does not cover
- [x] 115-17-PLAN.md — Gap 1, first restated copy. `tests/property_tests.rs` carried a five-entry mirror with no gate (WR-01) and an `arb_container()` drawing three of five (CR-01, WR-02), so `dependencies` was structurally unreachable in the generated space and rename invariance — the one fence here a RULE defect cannot satisfy — could not reach it. Widens the mirror and the container draw to six, adds a COMPILED mirror-equality gate against the seam, corrects WR-06's over-generalised strip justification, and runs three negative controls in three deliberately different configurations — including the BOTH-BLIND one where the surgical-scope and dialect-purity assertions are confirmed PASSING and only rename invariance fails, which is `D-115-AF`'s "check WHICH fence fired" applied
- [x] 115-18-PLAN.md — Gap 1, second restated copy plus the corpus. Brings `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` onto the six-keyword rule (closing the false-positive window 115-16 opens) and commits seed `15_dependencies_named_default`, CR-01's reproduction document. Keeps the copy an INDEPENDENT literal on measured grounds — correct-mirror-plus-blind-crate makes invariant 5 FIRE, derived-mirror makes the target exit 0 on the same seed. Observes invariant 5 (Control D) and, with invariant 5 silenced so it cannot mask, invariant 6 (Control E); and MEASURES the blind spot as Control F's exit 0 — the target cannot detect a keyword-list omission it SHARES with the crate. Also retires WR-03's two surviving copies of the retracted "TOTAL — no skip condition" claim. **Shipped:** the drift window was OPEN when the plan started and was observed CRASHING on CORRECT behaviour — exit 77, invariant 5, with `normalized to:` byte-identical to `Input was:` — then exit 0 after the widening. All three controls fired as specified with the FRAME named in each: Control D invariant 5 at `:642`, Control E invariant 6 at `:796` (`container: dependencies, name: default`) with invariant 5 silenced so it could not mask, and Control F exit **0** with nothing fired — the measured LIMIT, written in three places and attributed to `src`'s own-literal fence, which was itself run and OBSERVED to fail at `output_validation.rs:1429` in that same both-blind tree rather than merely named. 15 tracked seeds, `-runs=0` replay clean at 20 098 runs, a **3 614 479**-run `+nightly` campaign clean with an EMPTY artifacts dir (−2.3% vs 115-15, inside the ~3% widening cost `T-115-DEP-19` accepts). `src/server/output_validation.rs` handed on as a 0-byte diff, `shasum` OK
- [x] 115-19-PLAN.md — The recurrence pattern, Gap 2, and the booking. WR-01 found three literal copies of two keyword lists with **no gate that they agree**, which is the actual defect behind three rounds; `tests/keyword_list_mirrors.rs` is a featureless, gate-visible source-text gate over all three PLUS the meta-schema-derived expectation, so both silent drift modes (one copy lags; all three drift in lockstep) become loud. Gap 2 scopes `contracts/mcp-protocol-sdk-v1.yaml`'s `output_schema_draft_pin` equation head to the `walk:` clause five lines below it and rewrites the three `binding.yaml` note heads whose first sentence still carries the retracted unscoped total (WR-04). Then, and only after `make quality-gate` + the PR-blocking `pmat --checks complexity` both exit 0, books SCHM-01 on this round's measured evidence with the prior records amended, and triages EVERY round-3 review finding into `D-115-AH`/`D-115-AI` — the convention `115-VERIFICATION.md` booked as broken for the first time in three rounds. **Shipped:** the drift gate landed featureless and is CONFIRMED running inside `make quality-gate` (2 tests, visible in the gate transcript), with all three controls observed — one copy shortened NAMES that file (run twice, for the fuzz copy and the property copy); all three shortened in LOCKSTEP passes assertion 1 and fires the DERIVATION-anchored assertion 2, which is WR-01's second silent mode made loud; the constant renamed fails *"expected EXACTLY ONE definition … found 0"* rather than passing over an empty extraction. Gap 2 landed with both retracted-total carriers gone from the file, `SCHEMA POSITION` in the equation head, six keywords in the `walk:` clause / name-position invariant / POSTCONDITION, five `115-16 COMPLETENESS CORRECTION` notes in `binding.yaml`, and **0** `signature:`/`function:`/`module_path:`/`status:` lines in the diff — with both contract controls observed (a re-indented block scalar makes PyYAML exit non-zero naming the line while the line-wise bindings gate still reports 5 passed — the exact contrast that justifies the PyYAML check; a ghost `function:` value fails the bindings gate naming the symbol). **⚠ THE FIRST `make quality-gate` RUN EXITED 2 AND WAS NOT NORMALIZED:** two live-HTTP tests panicked at the PRE-EXISTING native-roots `.expect` (`streamable_http.rs:458`) on a macOS keychain `Os(Error { code: -36 })`; the identical binary passed standalone immediately after and the re-run of the whole gate exited **0** at 5060 passed / 0 failed / 81 ignored across 312 result lines. Booked as `D-115-AL`. `pmat quality-gate --checks complexity` **0 violations**; SCHM-02/03 unregressed at **78/78**; this round's counts 20 / 25 / 21 vs 18 / 2 all matched. SCHM-01 booked `[x]` AFTER those commands, prior records amended (`grep -c` of the downgrade heading word still 1, and that guard was itself exercised 2→1). Ledger continued at `D-115-AK`/`D-115-AL`, NOT the `AH`/`AI` this line originally named — both were consumed by 115-16 and 115-17, and `AJ` by 115-18; writing a duplicate would have broken the whole-ID check that is one of this plan's own criteria

### Phase 116: Auth Hardening SEPs

**Goal**: The v2 auth-hardening SEPs land as hand-rolled source changes to the existing OAuth stack — strict on v2, lenient on v1 — so existing deployments (Lambda `oauth_passthrough`, the Graph/M365 example, documented proxy exceptions) keep working. Fully independent — parallelizes with Phases 113-115.
**Depends on**: Phase 112 (era gate to keep v1 lenient; parallelizable with Phases 113-115)
**Requirements**: AUTH-01, AUTH-02, AUTH-03
**Success Criteria** (what must be TRUE):

  1. The OAuth callback validates RFC 9207 `iss` — strict whenever the authorization server advertises `authorization_response_iss_parameter_supported` or emits `iss`, with a present-but-mismatched `iss` rejected on every era and v1 leniency tolerating only an ABSENT `iss` (AUTH-01, as amended 2026-08-03 in `0aebf7f6`)
  2. Dynamic client registration sends and accepts `application_type` (AUTH-02)
  3. The remaining auth-hardening SEPs — credential storage keyed by `(issuer, account, server)` plus the TWO adopted clarifications SEP-2351 and SEP-2207, with SEP-2350 explicitly out of scope — are applied without breaking existing v1 OAuth deployments, and no `oauth2`/`openidconnect` crates are added to the core SDK (AUTH-03, as amended 2026-08-03 in `0aebf7f6`)

**Plans**: 16 plans in 10 waves *(planned 2026-08-02; REPLANNED 2026-08-03 after cross-AI review — see `116-REVIEWS.md`. Scope note: RFC 9728 Protected Resource Metadata discovery and the RFC 8707 `resource` parameter were DEFERRED by owner decision on 2026-08-02 and are not planned here; the RFC 8707 deferral is why the credential key was widened to `(issuer, account, server)` rather than bound by audience. RESEARCH amendments A1-A4 are authoritative corrections to CONTEXT decisions whose literal wording is unimplementable, and the plans implement the corrected shapes. Every verification block uses `--features full,oauth` — `oauth` is not in the `full` feature, so bare `make quality-gate` compiles none of this phase's code surface — every nextest filter uses `binary(...)`, never `test(/.../)`, which selects zero and exits 0, and every count is PARSED from the `Summary` line rather than tailed.)*

**Replan deltas (2026-08-03, owner decisions D-116-R1/R2/R3 plus Codex HIGH #3/#6/#7/#8/#9):** the credential key widened from `(issuer, account)` to `(issuer, account, server)` because two MCP servers sharing one authorization server and account otherwise collide; `CredentialStoreAdmin` added so cargo-pmcp's `auth status`/`logout --all` can be thin wrappers rather than a parallel implementation; the credential FORMAT and its schema 1→2 migration moved into the ungated pure tier and the gated file store split into new plan **116-16**; callback validation moved INSIDE the loopback listener so the served page and the redemption decision are consequences of one result; `offline_access` moved to the authorization request and never introduced at refresh; an explicit discovery outcome matrix added with anchor mismatch, over-cap body and malformed security metadata TERMINAL, plus a per-issuer successful-candidate cache; the OAuth contract equations authored in Wave 1 and resolved in Wave 10; and 116-15's gate policy split into REQUIRED-GREEN vs ACCEPTED BASELINE DELTA so it is executable.

Plans:
**Wave 1**

- [x] 116-01-PLAN.md — Wave 1. Phase baselines, contract-first AUTHORING, gate proof. No `src/` files. Discharges CLAUDE.md's contract-first mandate by authoring three OAuth equations in `contracts/mcp-protocol-sdk-v1.yaml` plus eight `status: planned` bindings BEFORE any implementation (116-15 resolves and flips them), and records the PMAT quality-proxy write workflow every source-touching plan follows. Also records the semver baseline against `b2bf9157`, the `make doc-check` error distribution as the named ACCEPTED BASELINE DELTA ANCHOR, the measured `--features full` vs `full,oauth` nextest A/B (0 vs 5 on `binary(oauth_dcr_integration)`), the wasm32 target probe RESEARCH assumption A5 left open, the dependency fence with Pitfall 6's precise oauth2 scoping and whether `Cargo.lock` is tracked, and the standard parsed-count verification snippet every later plan cites. Also OBSERVES the D-15 tripwire reporting the four auth files under a temporary widening, then reverts to a zero-byte diff — the pre-fix violation list 116-14 must drive to zero

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 116-02-PLAN.md — Wave 2. AUTH-01's semantics: three marker-const error identities (`ISS_MISMATCH_MARKER`, `STATE_MISMATCH_MARKER`, `REAUTH_REQUIRED_MARKER`) on `Error::Protocol` per amendment A2 — `Error::Authentication` is a bare-String tuple variant with no `data` member, so a marker there would make its own predicate return false — plus the ungated, wasm-clean `src/shared/oauth_validation.rs` holding `AuthorizationRequestRecord`, `IssPresence`, `validate_authorization_response` (the spec's normative 4-row table, state-then-iss-then-error ordering) and the pure D-04 precedence resolver. Property tests derive the no-normalization invariant from RFC 3986 §6.2.2-6.2.3 rather than restating the comparison operator
- [x] 116-03-PLAN.md — Wave 2. AUTH-02's carrier: `application_type` inherent accessors over `DcrRequest`/`DcrResponse`'s existing `#[serde(flatten)] extra` map (D-09), never a field — `DcrRequest` is public, all-pub-field and not `#[non_exhaustive]` with ten in-repo literal sites, so a field is `constructible_struct_adds_field` = MAJOR. Documented last-write-wins precedence with collision tests in both orders, and a wire-shape test proving the flatten carrier emits a top-level key

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 116-04-PLAN.md — Wave 3. The remaining pure primitives: SEP-2351's ORDERED candidate list (amendment A3 — the spec requires a probe sequence, and RESEARCH measured that D-13's literal append-to-insert swap 404s Microsoft Entra ID, whose URL is in this SDK's own doctest), the RFC 8414 §3.3 `issuer_matches_metadata` anchor comparison, and D-10's `derive_application_type` where a mixed `redirect_uris` vector is an explicit error. A property test asserts the appended form survives in every candidate list
- [x] 116-05-PLAN.md — Wave 3. AUTH-03's PURE storage tier: `CredentialKey` keyed by `(issuer, account, server)` per D-116-R1 — so SEP-2352's "MUST NOT reuse across authorization servers" AND the two-MCP-servers-one-authorization-server collision both hold by key shape rather than by enforcement code — plus `StoredCredentials`, `CredentialSnapshot`, `parse_credential_snapshot` and the schema 1→2 migration with its `MigrationReport`, all ungated and wasm-clean so a DynamoDB-backed platform store gets identical migration behavior to the CLI and the parser is fuzzable. Declares the narrow `CredentialStore` seam (with an atomic `save_with_issuer`, and still no refresh — Open Question 4) alongside `CredentialStoreAdmin` (D-116-R2: enumeration, delete-by-server, clear-all, migration report — without which 116-13's subcommands could not be thin wrappers). Ships `InMemoryCredentialStore` and wires a wasm32 build fence into the org-required `gate`

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 116-06-PLAN.md — Wave 4. Pitfall 1, the phase's most consequential finding: `fetch_discovery` validates the document's `issuer` against the issuer used to build the URL BEFORE the metadata escapes, because without it AUTH-01's whole comparison is anchored on an attacker-chosen value. Plus the ordered candidate probe, a new `AuthorizationServerExtras` sibling type carrying the RFC 9207 flag (amendment A1 — a field on `OidcDiscoveryMetadata` would be MAJOR), and a streaming two-refusal reqwest bounded-read helper. The lying-document fixture must be OBSERVED failing pre-fix
- [x] 116-08-PLAN.md — Wave 4. House ALWAYS requirements: a fuzz target over `validate_authorization_response` and `discovery_url_candidates` whose Ok-invariant is decoded INDEPENDENTLY inside the target, so it can see a rule defect the crate shares with a restating mirror; and `examples/c11_oauth_iss_state_validation.rs`, which actually RUNS the accept and both reject paths with no network, no browser and no `oauth` feature

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 116-07-PLAN.md — Wave 5. The same three changes for `generic_oidc.rs` and `cognito.rs`, which build the identical wrong URL; Cognito additionally never trimmed a trailing slash. Its TTL cache must keep short-circuiting and must never cache an anchor-rejected document. Explicitly does NOT touch the MCP resource-server side
- [x] 116-09-PLAN.md — Wave 5. AUTH-01 wired into the CLI flow: the per-request record built before the redirect, `state` bound via `generate_state()` (today it is an unnamed temporary at `:712`, and generated by the wrong RFC's helper), and validation performed INSIDE the loopback listener BEFORE the browser response is committed — the previous shape served HTML first, so a callback later rejected for `state`/`iss` had already displayed success and the required failure-HTML branch was unselectable. Adds the D-04 builder plus `PMCP_OAUTH_ISS_VALIDATION` (with an unrecognized value warned, not swallowed), a `BrowserLauncher` seam that makes the interactive flow testable end to end without opening a browser, and a bounded request-line read. Every rejection test asserts three things: the marker predicate, the FAILURE page bytes, and an `expect(0)` mock on `/token`
- [x] 116-16-PLAN.md — Wave 5. The gated `FileCredentialStore` over 116-05's pure format: cargo-pmcp's ported atomic 0o600-in-0o700 write, plus the concurrency discipline an atomic rename never provided — every mutation is a serialized read-modify-write under a `tokio::sync::Mutex` and an `O_EXCL` advisory lock with a 30s staleness break, and `save_with_issuer` is overridden as ONE atomic update so the store cannot name one issuer while holding another's credentials. Split from 116-05 because D-116-R1/R2 grew it past budget; adds no locking dependency and contains no knowledge of the JSON format

**Wave 6** *(blocked on Wave 5 completion)*

- [x] 116-10-PLAN.md — Wave 6. Two SEPs at one edit site: derived `application_type` sent unconditionally (D-11, with echo divergence warned and never fatal), `grant_types` gaining `refresh_token`, and `offline_access` handled at the RIGHT protocol stages — declared in DCR client metadata AND requested in the authorization request when advertised, with the GRANTED scope recorded from the token response (RFC 6749 §5.1 for the omitted-`scope` case) and never introduced at refresh. Plus an actionable registration-rejection error naming what was sent. Echo divergence is pinned by a private pure helper unit-tested inline and persisted through `StoredCredentials`' private fields, so no field is added to any public constructible type. SEP-837's optional retry MAY is deliberately not adopted

**Wave 7** *(blocked on Wave 6 completion)*

- [x] 116-11-PLAN.md — Wave 7. The store adopted: credentials, the DCR-issued `client_id` and the granted scopes keyed by `(issuer, account, server)` and persisted with `save_with_issuer` in one atomic update, the issuer-less legacy `oauth-tokens.json` never read and its discard announced once, and D-18's AS-change detection refined by amendment A4 to branch on credential provenance — warn-and-re-register for DCR, a typed `reauth_required` error for pre-registered, which is what the spec's two adjacent sentences actually prescribe

**Wave 8** *(blocked on Wave 7 completion)*

- [x] 116-12-PLAN.md — Wave 8. D-14's three refresh defects, all of which block headless operation: the stored refresh token destroyed whenever the AS omits one, DCR clients unable to refresh at all, and `scope` never sent — now sent as EXACTLY the stored granted scope or omitted entirely, never widened with an advertised-but-ungranted `offline_access` (RFC 6749 §6). Plus D-08's `Interactivity::RefreshOnly` making the browser path unreachable by construction instead of a five-minute wait on a listener nothing can reach, and this file's remaining hygiene — bounded reads, the post-hoc DCR cap upgraded to streaming, the plaintext token log, and the private PKCE duplicates

**Wave 9** *(blocked on Wave 8 completion)*

- [x] 116-13-PLAN.md — Wave 9. D-19's convergence: cargo-pmcp drops its parallel `TokenCacheV1`, keeps `oauth-cache.json` as the surviving path and migration source, and its five `auth` subcommands become thin wrappers over `CredentialStore` + `CredentialStoreAdmin` — with `logout`'s four semantics test-pinned first, and a logout-isolation test proving `auth logout <A>` no longer reaches a second server sharing the same authorization server. Scopes the no-oauth2 claim by keeping cargo-pmcp's pre-existing direct `oauth2 = "5.0"` confined to `deployment/`, and makes versions/pins coherent for publish — including the `Cargo.lock` the two version bumps rewrite
- [x] 116-14-PLAN.md — Wave 9. D-113-V closed by measurement: `EXTRA_SCOPE` and the `REQUIRED_FILES` anti-vacuity guard widened to the four auth files using FULL RELATIVE PATHS (a base-name entry for `auth.rs` could be satisfied by the wrong file), the tripwire reporting zero, `WHOLE_BODY_ALLOWLIST` still empty at its written floor, and the module doc naming AUTH-03/D-15 as second owner. Runs LAST among source-touching plans on purpose — widening earlier would leave the gate red for waves. THREE controls are RUN, not assumed, and the anti-vacuity one is run in the direction that can actually fail: scope removed while the requirement is retained. The reverse (requirement removed, scope intact) is run separately and recorded as the measured LIMIT, not as evidence

**Wave 10** *(blocked on Wave 9 completion)*

- [x] 116-15-PLAN.md — Wave 10. Every gate run at HEAD under an explicit TWO-CLASS acceptance policy — eleven REQUIRED-GREEN gates, and `make doc-check` alone as an ACCEPTED BASELINE DELTA whose condition is "no new errors vs the recorded anchor AND zero errors in any touched file" — stated before any number is recorded, so the previous revision's self-contradictory "stop if any gate is red while doc-check stays red" is gone. Then the Wave-1 contract bindings are resolved against real source and flipped `planned` → `implemented`, with every equation invariant mapped to a named test. Only then the D-20 bookings, each citing an artifact plus a named `binary(...)` and a PARSED non-zero count, made against the AMENDED AUTH-01/AUTH-03 text (`0aebf7f6`) so no booking has to narrate a gap between wording and code. Closes with a deferred-items register giving every deferral, amendment, limitation AND declined review finding a named owner

### Phase 117: Agents, Tester & v1 Severability

**Goal**: pmcp's own higher-level clients reach v2 (`pmcp-agent` incl. task polling, `mcp-tester` for dual-version testing), and v1-only machinery is isolated behind a clearly severable era-gated layer with a documented sunset policy — so a future major removal is a deletion, not a refactor — while the v2 path is simplified of session/SSE baggage.
**Depends on**: Phase 113 (Client v2), Phase 114 (Tasks v2 for agent task polling)
**Requirements**: CLNT-03, CLNT-04, SMPL-01, SMPL-02
**Success Criteria** (what must be TRUE):

  1. `pmcp-agent` (including its `ToolInvoker` and task polling) works end-to-end against a v2 server (CLNT-03)
  2. `mcp-tester` can exercise a v2 server (headers, discover, stateless flow) for dual-version testing (CLNT-04)
  3. v1-only machinery (initialize/session lifecycle, SSE resumability) is isolated behind a clearly severable era-gated layer with a documented legacy-support sunset policy — removal in a future major is a deletion, not a refactor (SMPL-01)
  4. The v2 code path carries no session/SSE-resumability baggage, and a simplification pass removes code the v2 model obsoletes wherever v1 compatibility permits (SMPL-02)

**Plans:** 14/14 plans complete

Plans:
**Wave 1**

- [x] 117-01-PLAN.md — Wave 1. `v1-compat` + `full-v2` features, the DERIVED full/full-v2 drift tripwire, and the condition-gated sunset policy wired into the blocking rustdoc gate (SMPL-01; D-01/D-02/D-04; re-confirms A-A1)
- [x] 117-02-PLAN.md — Wave 1. Golden v1 wire fixtures captured BEFORE the cut — body bytes AND headers, read through a bounded frame-counting SSE reader rather than the read-to-EOF helper: initialize, `Mcp-Session-Id` emission, `Last-Event-ID` replay, GET/DELETE (SMPL-02)
- [x] 117-03-PLAN.md — Wave 1. `mcp-tester` single-run report goldens captured against 0.7.0 as it stands, plus the A2 re-measurement that bounds what 117-11 may add (CLNT-04; D-11/A-D11/A-CI)
- [x] 117-04-PLAN.md — Wave 1. First `pmcp-agent` live-server harness (incl. a GUARANTEED task-associated tool result) + the four CLNT-03 cases written RED: v2 e2e, unconditional task polling to terminal, v1 fallback, unreachable-host propagation (CLNT-03; D-07/A-D08/D-09)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 117-05-PLAN.md — Wave 2. Blocking `v1-severance` CI job + all THREE `gate` edits, proved from the workflow file by a `serde_yaml` tripwire (no undeclared PyYAML) with `feature-flags` as its live negative control (SMPL-01; A-CI)
- [x] 117-06-PLAN.md — Wave 2. Prove the repo's FIRST `#[cfg_attr(…, path = …)]` paired module on a minimal payload, whole-file gate `src/shared/event_store.rs`, add the SEMANTIC null-twin check (no state held, no state/header operation, nothing declared the real module lacks) (SMPL-01/SMPL-02; D-03/A-D03)
- [x] 117-07-PLAN.md — Wave 2. Two-attempt era-pinned `client_for` classified by a TYPED reachability outcome built before stringification, additive `EffectTrace` version field plumbed end-to-end, deterministic `ReplayInvoker` era-mismatch guard, `pmcp-agent` 0.2.0 with both workspace pins updated (CLNT-03; D-07/D-08/A-D08/D-09)
- [x] 117-08-PLAN.md — Wave 2. The 14-entry expected-difference baseline as reviewable YAML (no new dep), its non-vacuity tripwire, and the ALWAYS fuzz target (CLNT-04; D-06)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 117-09-PLAN.md — Wave 3. `ServerState` collapses to one `V1State` (a ZST on `full-v2`); the seven era chokepoints move with signatures intact (SMPL-02; D-03/D-10)
- [x] 117-10-PLAN.md — Wave 3. The ALWAYS runnable example `s53_v2_agent_client`, placed where `make test-examples` actually builds it (CLNT-03)
- [x] 117-11-PLAN.md — Wave 3. Opt-in `--dual-run`, `run_dual` wrapping the existing orchestrator twice, era-observation probes emitting stable IDs (a `TestReport` carries no wire facts), era-aware `core_domain.rs` C-01/C-04 with no synthesised `InitializeResult`, baseline-driven `DualRunReport` in a NEW top-level struct (CLNT-04; D-05/D-11/A-D11)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 117-12-PLAN.md — Wave 4. The session-lifecycle and SSE-replay bodies move; the `full-v2` twin has NO `Last-Event-ID` reader at all, preserving the T-113-29/30 ordering structurally (SMPL-02; D-03/D-10)

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 117-14-PLAN.md — Wave 5. CLIENT-side severance: the transport's session lifecycle, stored session id, DELETE teardown and resumption surface gated; `LAST_EVENT_ID` co-gated with its one client reader; A4 measured for `MCP_SESSION_ID`; `Client::initialize` severability measured; derived inventory tripwire + a runtime proof RUN on `full-v2` (SMPL-01/SMPL-02; D-03/D-10)

**Wave 6** *(blocked on Wave 5 completion)*

- [x] 117-13-PLAN.md — Wave 6. GET/DELETE SPLIT so the v2 405 stays reachable — proven by a test EXECUTED under `--no-default-features --features full-v2`; plan 117-12's two deferred functions resolved; config-field gating (with a documented fallback); sunset policy reconciled to the code on BOTH server and client (SMPL-01/SMPL-02; D-03/D-10/A-D03)

### Phase 118: Conformance Against the Official Suite

**Goal**: The dual-version claim is validated by construction — the official conformance suite plus the extended Rust harness both run against whatever the dual-version binary actually does, with v1 fixtures kept green and deprecated capabilities verified still-functional under v2. Runs last, over the union of all prior work.
**Depends on**: Phases 112-117
**Requirements**: CONF-01, CONF-02, CONF-03
**Success Criteria** (what must be TRUE):

  1. The official `@modelcontextprotocol/conformance` suite (pinned to a commit, re-pinned after the final spec) runs in CI against a dual-version pmcp server example over real HTTP (CONF-01)
  2. The Phase-109 Rust conformance harness gains v2 fixtures while v1 fixtures stay green (dual conformance), verified with a dev-dependency-free build to avoid feature-unification false-greens (CONF-02)
  3. Deprecated Roots/Sampling/Logging capabilities remain fully functional under v2 negotiation (advisory-only deprecation, 12-month window) (CONF-03)

**Plans**: 10 plans

*Replanned 2026-08-09 after `/gsd-review --codex --gemini` (`118-REVIEWS.md`). Codex, reading
repository source, found three architectural blockers that invalidated the CONF-02/CONF-03 track;
all three were confirmed against source and locked as D-16/D-17/D-18. The era comparison now runs on
the PORTED Phase-117 probe machinery over real streamable HTTP (both eras, same transport); the 33
in-process fixtures become a v1-only regression guard; CONF-03 is proved by probes plus typed
completion round trips, not by an extended fixture format. Plan 118-10 was added for the D-08 rename,
the exact-count regression guard and the D-19 plan-lint.*

Plans:
**Wave 1**

- [x] 118-01-PLAN.md — Wave 1. D-13 + **D-18**: relax `Mcp-Name` to name-bearing methods only AND route the predicate through the COMBINED `name_bearing_key` table so `tasks/*` is validated as well as emitted; live-HTTP proof, byte-level FUZZ arm, contract-first equation (CONF-01)
- [x] 118-02-PLAN.md — Wave 1. D-01: the pinned `conformance/` Node manifest + lockfile + `.npmrc` `engine-strict`, `--ignore-scripts`, the ship-both-or-exclude-both packaging disposition, and the ONE canonical zero-check policy + two-pin reconciliation in `conformance/README.md` (CONF-01)
- [x] 118-03-PLAN.md — Wave 1. **D-16**: port the Phase-117 era substrate into team-servers — typed observations (`ObservationId`/`ObservedValue`/`PROBE_REGISTRY`, 14 ids), the baseline model + total parser + bidirectional `compare_eras`, the spec-artifact YAML, the schema gate with BOTH coverage directions, and the ALWAYS fuzz target (CONF-02/CONF-03)
- [x] 118-10-PLAN.md — Wave 1. D-08's prose-only fixture-format rename, the v1 regression guard tightened to EXACT counts (`failed == 0`, 33 total, exact per-directory + an on-disk file fence), and the **D-19** plan-lint that fails a piped build/test verification without `pipefail` (CONF-02)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 118-04-PLAN.md — Wave 2. D-05: the dual-version conformance example `s54_v2_dual_conformance` **registered as a `[[example]]` with `required-features`**, the full 2025-11-25 fixture surface measured green, output under `target/conformance-results/` (CONF-01)
- [x] 118-06-PLAN.md — Wave 2. **D-16/D-17**: the raw streamable-HTTP probe client, the dual-accept-list era target on an ephemeral port (incl. the three deprecated-capability tools), and `observe()` — fourteen ESTABLISHED observations under both eras over ONE endpoint (CONF-02/CONF-03)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 118-05-PLAN.md — Wave 3. D-13/D-18's follow-up re-measure, the MRTR + `InputRequiredResult` v2 surface, and both requirement sets green from ONE unrestarted process started from the BUILT binary, with a cross-era bleed probe (CONF-01)
- [x] 118-07-PLAN.md — Wave 3. D-10/D-11/D-12 via **D-17**: the era matrix with a bidirectional baseline join, Roots/Sampling/Logging COMPLETING under both eras through a `StreamableHttpTransport` typed client, the baseline reconciled from measurement, and the 12-month advisory MECHANISM window reconciled into `docs/v1-sunset-policy.md` (CONF-02/CONF-03)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 118-08-PLAN.md — Wave 4. The two driver scripts (one process from the built binary, two runs, readiness poll, process-GROUP teardown, per-run + total timeouts, the EXECUTABLE zero-check gate and the check floors; two dev-dependency-free build fences + nonzero-count guards for all three harness targets) with commands as DATA, plus the two Makefile targets (CONF-01/CONF-02/CONF-03)

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 118-09-PLAN.md — Wave 5. D-02/D-15: two BLOCKING CI jobs (Node 22 + lockfile-keyed npm cache + `timeout-minutes`), all FOUR `gate` wirings each, and a `serde_yaml` structural tripwire proving a **BIJECTIVE** job→env→conditional→echo mapping, with a live negative control, the two-pin binding, and seven executed failure demonstrations (CONF-01/CONF-02/CONF-03)

### Phase 118.1: Close the nine conformance gaps G-1..G-9 found by the official suite (INSERTED)

**Goal:** Both official-suite legs measure strictly better than the Phase-118 baseline (`2025-11-25`: 51 passed, 15 failed, exit 1, 11 scored scenarios red, 66 checks; `2026-07-28`: 124 passed, 54 failed, exit 1, 7 scored red, 178 checks), each of G-1..G-9 carries an explicit **FIXED / REFUTED / DEFERRED** disposition backed by a named RED-to-GREEN artifact, and the blocking CI gate is widened to exactly the surfaces that then pass — with no `--expected-failures`, no allowlist and no known-failure baseline.
**Requirements**: CONF-04, CONF-05, CONF-06, CONF-07, CONF-08
**Depends on:** Phase 118
**Plans:** 14/14 plans complete

**Success Criteria** (what must be TRUE):

  1. An embedded resource in a tool result or a prompt message serializes as the spec `EmbeddedResource` shape — `type: "resource"` with contents nested under `resource` — on both eras, binary content carries `blob` in both the nested and the flat `ReadResourceResult.contents` positions, content-level `annotations` is carried, and pmcp parses both the nested and the legacy flat shape while emitting only the nested one (CONF-04)
  2. `completion/complete` is served by a registered handler seam on both native dispatchers, and all five methods absent from the 2026-07-28 core schema answer HTTP 404 with `-32601` on v2 even with well-formed params while still answering normally on v1 (CONF-05)
  3. A v2 request missing `params._meta`, `io.modelcontextprotocol/protocolVersion` or `io.modelcontextprotocol/clientCapabilities` is rejected `-32602` + HTTP 400 while a missing `clientInfo` is served 200; version disagreement answers `-32020` and an unsupported agreed version `-32022` with `data.supported`; and `server/discover` emits `supportedVersions` from that same accept list (CONF-06)
  4. The server-to-client back-channel works over StreamableHTTP — `peer.sample()`, `peer.list_roots()` and `peer.elicit()` complete over v1 stateful HTTP without blocking concurrent requests, progress notifications reach the client on both eras, and `set_result_meta` survives the `ToolOutput::Result` verbatim path (CONF-07)
  5. `RequestHandlerExtra::client_capabilities()` returns the capabilities a v1 client advertised in its `initialize` handshake at every handler-dispatch construction site (CONF-08)

Plans:

**Wave 1**

- [x] 118.1-01-PLAN.md — Wave 1. Bookkeeping plus the **D-14** branch precondition: mint CONF-04..CONF-08 as scoreable wire behaviour (D-12), fill this roadmap entry, and land Phase 118 on `main` before anything is re-measured (CONF-04, CONF-05, CONF-06, CONF-07, CONF-08)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 118.1-02-PLAN.md — Wave 2. **D-04**: CONF-04's RED fences — spec-derived byte goldens for the embedded-resource tool-result and prompt-message positions on BOTH eras plus the tolerant-reader fuzz target, each demonstrated RED against the unfixed tree (CONF-04)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 118.1-03-PLAN.md — Wave 3. **D-01/D-03/D-15**: the CONF-04 fix — nested `EmbeddedResource` emitter, tolerant flat-input reader, `blob`, `annotations`, `#[non_exhaustive]` plus constructors in ONE batched edit, and the D-02 CHANGELOG wire-change callout with its documented `cargo semver-checks` delta (CONF-04)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 118.1-04-PLAN.md — Wave 4. G-4: the `completion/complete` handler seam on both native dispatchers, replacing the catch-all `json!({})` (CONF-05)
- [x] 118.1-05-PLAN.md — Wave 4. G-5: method-string retirement at the v2 ingress so all five schema-absent methods answer `-32601` under well-formed params — never validated against the suite's two false greens (CONF-05)

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 118.1-06-PLAN.md — Wave 5. G-6/G-8: the three-way `_meta` classifier (`-32602` + HTTP 400 for the two required keys, HTTP 200 for a missing `clientInfo`) and the `-32020` / `-32022` check ordering (CONF-06)
- [x] 118.1-07-PLAN.md — Wave 5. G-7: `server/discover` emits `supportedVersions` from the SAME accept list the version errors are computed from (CONF-06)

**Wave 6** *(blocked on Wave 5 completion)*

- [x] 118.1-08-PLAN.md — Wave 6. G-9: v1 capability plumbing, so `client_capabilities()` carries the `initialize` handshake's capabilities at every handler-dispatch construction site (CONF-08)

**Wave 7** *(blocked on Wave 6 completion)*

- [x] 118.1-09-PLAN.md — Wave 7. **D-06/D-07**: `PeerHandle::elicit` plus the `set_result_meta` drain on the `ToolOutput::Result` verbatim path (CONF-07)

**Wave 8** *(blocked on Wave 7 completion)*

- [x] 118.1-10-PLAN.md — Wave 8. G-3, part 1: the v1 server-to-client channel plus inbound routing before the server mutex (CONF-07)

**Wave 9** *(blocked on Wave 8 completion)*

- [x] 118.1-11-PLAN.md — Wave 9. G-3, part 2: v1 session-bound peer injection, progress notifications, and the 118-07 `capability-not-offered` era-matrix tripwire flips (CONF-07)

**Wave 10** *(blocked on Wave 9 completion)*

- [x] 118.1-12-PLAN.md — Wave 10. G-3, part 3 (**D-16**): v2 multi-frame SSE progress on the POST response body — notification frames then the result frame ONLY, never an independent server-to-client request (CONF-07)

**Wave 11** *(blocked on Wave 10 completion)*

- [x] 118.1-13-PLAN.md — Wave 11. Re-measurement number one at the HELD `0.2.0-alpha.11` pin, plus the **D-10** FIXED/REFUTED/DEFERRED disposition for each of G-1..G-9 AMENDED into `118-CONFORMANCE-GAPS.md` (CONF-04, CONF-05, CONF-06, CONF-07, CONF-08)

**Wave 12** *(blocked on Wave 11 completion)*

- [x] 118.1-14-PLAN.md — Wave 12. **D-09**: gate widening to exactly the surfaces that then pass — no `--expected-failures`, no allowlist, no known-failure baseline — plus the **D-08** re-pin and re-measurement number two, reported as a SEPARATE delta from the fixes' delta (CONF-04, CONF-05, CONF-06, CONF-07, CONF-08)

### Phase 118.2: The v1 client SSE transport and the `notifications/message` emitter (INSERTED)

**Goal:** Close the two residuals Phase 118.1 measured and could not close within its own scope, so that the server-to-client channel 118.1 built is usable end to end by pmcp's OWN client, and a tool handler can emit MCP log notifications. Both were signed off as **OPEN** sub-items of G-3 at plan 118.1-13's D-10 gate (2026-08-11); neither is a re-litigation of a closed gap.
**Requirements**: CONF-09, CONF-10 (minted 2026-08-11 at planning time per D-17; rows added to `REQUIREMENTS.md`'s checklist AND traceability table so the existing 10-orphan-ID warning is not widened)
**Depends on:** Phase 118.1
**Plans:** 17/17 plans executed in 7 waves (plan 02 was merged into plan 01 during the cross-AI review round; the numbering gap at 02 is deliberate), plus 5 GAP-CLOSURE plans (14-18) in waves 8-11 addressing the safety truth `118.2-VERIFICATION.md` failed, plus 3 SECOND-ROUND gap-closure plans (19-21) in waves 12-14 closing the two Critical defects that closure's own code introduced, plus an unplanned THIRD round (commits `e104dea6`, `d01b87e2`, `2d385d60`, `26447f94`) that shipped the per-id response router with NO plan and NO summary, plus 4 FOURTH-ROUND gap-closure plans (22-25) in waves 15-18 closing the two defects the third round left behind, hardening the two transport-wide sequences the send-path fix makes concurrently reachable, and giving that round a record — 24 plans total, 20 executed and 4 planned

- [x] 118.2-13-PLAN.md

**Why these two, and why together.** Both are the same shape: the v1 server-to-client channel exists and is proven, and each of these is a missing surface at one end of it. The developer chose one combined phase over two at the 118.1-13 sign-off.

**Success Criteria** (what must be TRUE):

  1. pmcp's own `StreamableHttpTransport` client can hold a live GET SSE stream and consume server-initiated messages over v1 HTTP. **Measured reason it cannot today:** `StreamableHttpTransport::start_sse` (`src/shared/streamable_http.rs`) calls `collect_body_within_cap` — a WHOLE-BODY read — before handing the result to `SseParser::feed_complete_body`; its own rustdoc says "this body was already read into memory in one piece, not a chunk of a live stream". A v1 session SSE stream never ends, so there is nothing to collect, the client registers no receiver, and the server's `route_to_session_stream` finds no stream for the session. **The SERVER half is sound and is NOT in scope here** — `binary(http_peer_roundtrip)` is 12 tests run, 12 passed, including `http_peer_sample_completes_over_a_v1_session`, `http_peer_list_roots_completes_over_a_v1_session` and `http_peer_elicit_completes_over_a_v1_session`, whose client is a raw TCP reader that DOES hold the stream open. That contrast is what localises the defect to the client. Consequence today: `binary(era_matrix)`'s `deprecated_capabilities_complete_under_both_eras` asserts `no-live-stream` rather than `completed`
  2. A tool handler can emit MCP `notifications/message` log records during a call, and they reach the client on both eras. **Measured reason it cannot today:** there is no handler-facing emitter. `PeerHandle` (`src/shared/peer.rs:74`) is exactly `{ sample, sample_with_tools, list_roots, elicit, progress_notify }`; `RequestHandlerExtra` exposes `report_progress` / `report_percent` / `report_count` and no logging analogue; and `ServerNotification::LogMessage` (`src/types/notifications.rs:135`) is constructed ONLY in tests (`src/types/notifications.rs:278`, `src/types/subscriptions.rs:920`, `src/server/subscriptions.rs:1459`) — no production path emits it. **Evidence:** the official suite's `tools-call-with-logging` scenario fails `No log notifications received` on the `2025-11-25` leg, and it is the ONE remaining gap-attributable failure across both legs after Phase 118.1 (`GAP_ATTRIBUTABLE_FAILURES = 1`). Closing it should take the v1 leg to zero scored failures
  3. Both changes are additive to public API surface and carry a `cargo semver-checks` verdict recorded in the phase's summary — adding a `PeerHandle` trait method is a breaking change for external implementors unless defaulted, which is why this is its own phase rather than a fix folded into 118.1
  4. The official suite is re-measured after the change and the delta is reported against Phase 118.1's closing numbers (`2025-11-25`: 72 passed / 2 failed / 74 checks / exit 1; `2026-07-28`: 142 passed / 36 failed / 178 checks / exit 0), at whatever pin is then current

**Plans**:

**Wave 1** *(parallel — the client half and the emitter half are independent until the joint fence)*

- [x] 118.2-01-PLAN.md — Wave 1. **Defect A + Defect B on the GET path, in ONE atomic slice**: the dead `202 Accepted` branch means pmcp's client never issues a GET at all (MEASURED: 2 POSTs, 0 GETs), AND the whole-body collect means an opened stream is never read — `start_sse` collects before it spawns (`:1002` vs `:1020`), so the two cannot land separately. Ships the recording-TCP-listener harness plans 03/04 reuse, the `Result`-carrying bounded receive channel that gives terminal reader errors a route to `receive()`, the incremental reader, and the bounded-reads ALLOWLIST entry (CONF-09)
- [x] 118.2-05-PLAN.md — Wave 1. **D-06/D-08/D-09/D-12**: `extra.log(..)` + `extra.log_with_data(..)` on `RequestHandlerExtra` — two methods, no `PeerHandle` trait method — with `LoggingLevel` syslog ordering and the no-sink `Ok(())` contract (CONF-10)

*(plan 02 was merged into plan 01 by the cross-AI review round and deleted; the numbering gap is deliberate)*

**Wave 2** *(blocked on Wave 1)*

- [x] 118.2-03-PLAN.md — Wave 2. **D-01, second site**: the POST-response `text/event-stream` read — the one that deadlocks in-tool elicitation — plus retiring `SseParser::feed_complete_body` and all three of its co-located dependants, and the streaming response-middleware contract (CONF-09)
- [x] 118.2-06-PLAN.md — Wave 2. **D-07**: `attach_request_log_sink`, the ONE unit both native dispatch roots call, twinned on 118.1's `attach_request_peer` — plus the `ProtocolContext.resolved_log_level` carrier that gets a level from the HTTP ingress to the dispatch root that actually builds the `RequestHandlerExtra` (CONF-10)

**Wave 3** *(blocked on Wave 2)*

- [x] 118.2-04-PLAN.md — Wave 3. **D-03**: bounded reconnect with `Last-Event-ID` over an owned `SseReaderContext`, through a paired cursor accessor and with no second call-site `#[cfg]`; cancellation on close/drop during backoff; the public `decode_sse_chunks_for_fuzz` seam and its target; the `full-v2` severance build and the `cargo semver-checks` verdict (CONF-09)
- [x] 118.2-07-PLAN.md — Wave 3. **D-10/D-11/D-12**: the v1 per-session level in `V1State` with its `full-v2` null twin, the first real reader of `io.modelcontextprotocol/logLevel`, the resolved level written onto `ProtocolContext` at BOTH ingress paths, and ignore-and-default on malformed input (CONF-10)

**Wave 4** *(blocked on Wave 3)*

- [x] 118.2-08-PLAN.md — Wave 4. **D-13**: `logging/setLevel` split out of the four-method residual arm — literal `{}` on v1 (MEASURED suite constraint), `-32601` on v2 — on BOTH roots (CONF-10)
- [x] 118.2-09-PLAN.md — Wave 4. The ALWAYS-requirement example `s55_handler_logging`, plus rewriting the dual-conformance fixture's logging arm from `tracing::info!` (which never reaches the wire) to `extra.log(..)` inside the suite's 200 ms budget (CONF-10)

**Wave 5** *(blocked on Wave 4 — needs BOTH halves)*

- [x] 118.2-10-PLAN.md — Wave 5. **D-15.3** the joint fence (pmcp on BOTH ends of a live stream, asserted at the transport layer with zero new public API) and **D-15.1** the `era_matrix` flip: constant AND prose, with ERA-12 undisturbed (CONF-09, CONF-10)

**Wave 6** *(blocked on Wave 5)*

- [x] 118.2-11-PLAN.md — Wave 6. **D-15.2/D-16**: re-measurement at the HELD `0.2.0-alpha.11` pin, then the first era leg gated on its own EXIT CODE — with the per-revision scored floor split and the blocking list WIDENED rather than shortened — plus the CONF-09/CONF-10 bookings (CONF-09, CONF-10)

**Wave 7** *(blocked on Wave 6)*

- [x] 118.2-12-PLAN.md — Wave 7. **D-14**: the re-pin as the FINAL act, re-measured once, with the bump's delta reported SEPARATELY from the fixes' delta — and a blocking developer checkpoint if the bump reds the D-16 gate (CONF-09, CONF-10)

**GAP CLOSURE (planned 2026-08-17).** `118.2-VERIFICATION.md` returned `gaps_found`: the four literal Success Criteria above are achieved and corroborated by the official suite, but the phase's own prose goal — "usable end to end by pmcp's OWN client" — is not safely true. Two Critical defects (CR-01, CR-02) and two Warnings (WR-01, WR-02) from `118.2-REVIEW.md` live in exactly the client code this phase shipped, and were independently confirmed against the merged source. These five plans close them. CONF-09 and CONF-10 stay booked **Complete** as literally worded; plan 18 AMENDS CONF-09's traceability evidence rather than minting a new requirement ID (D-17). WR-03..WR-06 and IN-01..IN-06 are explicitly OUT of scope and are recorded by plan 16.

**Wave 8**

- [x] 118.2-14-PLAN.md — Wave 8. **CR-01, the tracer slice**: `MIN_SSE_RECONNECT_DELAY` floors a peer-supplied `retry: 0`, and the reconnect budget is refunded only on sustained uptime rather than on a single delivered frame — plus the delivered-arm fence the existing 14 fences structurally cannot reach (CONF-09)

**Wave 9** *(blocked on Wave 8 — same file)*

- [x] 118.2-15-PLAN.md — Wave 9. **CR-02**: terminal reader errors move off the response FIFO onto a private sticky, write-once latch surfaced only behind an empty queue, and `Client::dispatch_request` correlates `response.id` against the awaiting `request_id` — with the idle-poisoning fence and the desync fence, both driven through a real `pmcp::Client` (CONF-09, CONF-10)

**Wave 10** *(parallel — plan 16 touches no source file)*

- [x] 118.2-16-PLAN.md — Wave 10. The record: WR-03..WR-06 and IN-01..IN-06 appended to `deferred-items.md` with their review anchors and reasons, and the reasoned no-external-API `COVERAGE.md` declaration (CONF-09, CONF-10)
- [x] 118.2-17-PLAN.md — Wave 10. **WR-01 + WR-02**: a `watch` shutdown signal raced against the parked body read at both SSE sites (closing the idle-stream task/socket leak on drop AND on `close()`), and the resumption cursor promoted to per-reader state so a POST-stream id can never become the session GET's `Last-Event-ID` (CONF-09)

**Wave 11** *(blocked on Waves 8-10)*

- [x] 118.2-18-PLAN.md — Wave 11. The close-out: CONF-09's traceability row AMENDED in place with the four fixes and their measured fence counts, the two consumer-observable behaviour changes disclosed in `WINDOWS.md`, and the phase's single authoritative `cargo semver-checks` verdict plus a D-16 regression check framed as server-side only (CONF-09, CONF-10)

**SECOND GAP CLOSURE (planned 2026-08-17).** The re-verification returned `gaps_found` again, 4/5. The first round genuinely closed CR-01 and WR-02, but its OWN code for CR-02 introduced two NEW Critical defects on the same client path — the sticky, unresettable latch pre-empting an in-flight SSE-answered POST response (permanently, for the life of the process), and an id-mismatch discard that holds the transport write lock across a wait bounded by a timeout `pmcp::Client` does not have — plus a documentation-of-record error that books the second as bounded. Plans 19-21 close both, add the SSE-answered-POST fence shape the existing fences miss, and correct every document that carries the false premise.

**Wave 12**

- [ ] 118.2-19-PLAN.md — Wave 12. **BLOCKER 1, the tracer slice**: the terminal reason gains the identity of the stream that raised it, `drain_or_latch` never surfaces a latch while a POST-response reader is live, and a successful `start_sse` re-open CLEARS the latch — fenced by the SSE-answered-POST shape fence 16 could not reach (CONF-09)

**Wave 13** *(blocked on Wave 12 — same test file)*

- [ ] 118.2-20-PLAN.md — Wave 13. **BLOCKER 2, both halves independently fenced**: the discard wait gains a real ceiling (`MISMATCH_DISCARD_TIMEOUT`) and a discard cap (`MAX_ID_MISMATCH_DISCARDS`), the transport write guard is released on a `MISMATCH_RECEIVE_SLICE` so one bad frame can no longer wedge the whole `Client`, and the in-code claim of a caller-supplied timeout is corrected (CONF-09)

**Wave 14** *(blocked on Waves 12-13)*

- [ ] 118.2-21-PLAN.md — Wave 14. The record: the false ceiling premise REMOVED in place from `deferred-items.md`, CONF-09's limitation (vi) corrected and BLOCKER 1 disclosed as (vii), `WINDOWS.md` entries 12/13 restated at the severity found and entry 16 cross-referenced, every declined finding named with an owner, and the round's closing semver, gate and D-16 verdict (CONF-09, CONF-10)

**Wave 15** *(FOURTH gap-closure round, against `118.2-VERIFICATION.md` re-verified 2026-08-20 at HEAD `5caeed05`)*

- [ ] 118.2-22-PLAN.md — Wave 15. **CR-02's surviving half**: the ceiling that re-arms itself across calls from the debris of the call it just killed. A bounded, locally-minted-ids-only ledger lets the pump tell OUR OWN abandoned request's late answer from the peer's mis-addressed frame, so a dead call no longer charges the next one's budget. Fenced deterministically by COUNT, not by clock (CONF-09)

**Wave 16** *(blocked on Wave 15 — same test file. Lands BEFORE the guard is removed, deliberately: serialising what the outer guard already serialised is a no-op, so this order costs nothing while the reverse leaves a commit in which both hazards are reachable and unprotected)*

- [ ] 118.2-25-PLAN.md — Wave 16. **The two transport-wide sequences the client's outer guard was silently protecting**, both reachable TODAY through two clones of one transport: the 401 `on_unauthorized` purge-and-refresh over a transport-wide `AuthProvider` (concurrent 401s destroy a rotating refresh token — a recorded blocker for the durable-agent shape), and `start_sse`'s abort→open→reset→respawn, which is a transport-wide read-modify-write across an await and can strand a reader `close()` cannot reach. Single-flight plus generation check, and an atomic restart (CONF-09)

**Wave 17** *(blocked on Wave 16)*

- [ ] 118.2-23-PLAN.md — Wave 17. **The wedge that moved to the SEND path** (new finding, seen by neither review nor prior verification): `dispatch_request` holds the transport write guard across the whole POST, and there is no request timeout anywhere, so a peer that accepts the POST and never writes response headers freezes every operation on the `Client`. Closed by an owned shared-send handle — an additive, defaulted `Transport` accessor — NOT by a deadline, which would fail legitimate long calls against JSON-answering and Lambda-hosted servers (CONF-09)

**Wave 18** *(blocked on Waves 15-17)*

- [ ] 118.2-24-PLAN.md — Wave 18. The record, fourth time: `WINDOWS.md` entry 20's false "per-id routing removes this too" claim REMOVED in both representations, entry 13 re-pointed at symbols that resolve, entries of record minted for the unplanned THIRD round and for plans 22/23 including plan 23's two accepted residuals, the CONF-09 row's five deleted identifiers replaced, the migration chapter brought into agreement with the shipped client, the five stale fence-calibration comments corrected, the `PooledTransport` residual named rather than silently inherited, and semver plus both era legs re-measured at the tree they describe (CONF-09, CONF-10)

**Not in scope:** the `json_schema_2020_12_tool` and `x-mcp-header` fixture gaps on the dual-conformance example, and the Tasks-extension surface — all three are missing FIXTURES rather than SDK defects, and all are classified as such in `118-CONFORMANCE-GAPS.md`'s amendment. Also not in scope: the `ServerAcceptsWhitespaceHeaderValue` flake, which was REFUTED as an SDK defect (the server trims OWS correctly in 14/14 fresh processes) and is a suite-side check-design issue.

### Phase 119: Documentation — Three Shapes + v2 Migration

**Goal**: The milestone is documented per the house three-shapes rule (pmcp-book chapters + runnable examples + README/course), leading with the `cargo pmcp` workflow — covering both the v2.4 Agents & Teams surface (carried from Phase 111) and the v2 dual-version migration story, with runnable v2 examples verified against the shipped code.
**Depends on**: Phases 112-118 (DOCS-05/06 document shipped v2 code; DOCS-04 has no v2 dependency and may land early)
**Requirements**: DOCS-04, DOCS-05, DOCS-06
**Success Criteria** (what must be TRUE):

  1. Agents & Teams are documented in three shapes (pmcp-book chapters, runnable examples, README/course), cargo-pmcp-first — carried from v2.4 Phase 111 (DOCS-04)
  2. A v2 migration guide + dual-version documentation ships: how to opt into v2, the dual-version story, Tasks extension migration, and the legacy sunset policy (DOCS-05)
  3. Runnable v2 examples ship and pass: a stateless (Lambda-style) v2 server and a v2 client/agent example (DOCS-06)

**Plans**: 10/10 plans executed across 5 waves (tracer-first; D-14 before D-13; sentinel before tripwire)

Plans:
**Wave 1**

- [x] 119-01-PLAN.md — Task zero: discharge the Phase-113 arm-1 hold, upgrade the `## Verdict` to `PUBLISHED-CONFIRMED`, flip the eleven HTTP/CLNT requirements (D-01, one-way, checkpoint-gated)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 119-02-PLAN.md — TRACER: "Agents as MCP Clients" end-to-end across three shapes — chapter + SUMMARY re-parent + README section + run-test helper + `mdbook build` negative control

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 119-03-PLAN.md — Example-build gate: baseline the pre-existing surface (D-14), then make `make test-examples` strict and observe it red (D-13)
- [x] 119-04-PLAN.md — Example run-test completion: the remaining DOCS-04 legs plus the DOCS-06 s47/s48/s53 socket leg (D-15)
- [x] 119-05-PLAN.md — v2 migration chapter by role (server/client/agent) + the `[CONSUMER-OBSERVABLE]` disclosure sentinel (D-02/D-03a/D-04/D-05/D-06/D-12)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 119-06-PLAN.md — pmcp-course Part VIII ch24 Agents & Teams chapter + tiered exercises (D-09). Moved out of Wave 3 by the cross-AI review: its Exercise 2/3 pass predicates must name the same banners `tests/docs04_examples_run.rs` asserts, and 119-04 writes two of those legs
- [x] 119-07-PLAN.md — Agent Teams book chapter, completing Phase 111's three named chapters (D-08)
- [x] 119-08-PLAN.md — In-place era amendments: `ch12-7-tasks.md` with a provisionality callout (D-07) and the two `ch10` transport chapters, no code block touched (D-16)
- [x] 119-09-PLAN.md — README `## Protocol Versions`, refreshed release header, extended Examples block, CHANGELOG heading fixes (D-11)

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 119-10-PLAN.md — Disclosure tripwire (D-03b) + the full phase gate (quality-gate, both `mdbook build`s, ledger parse, packaging review)

## Progress — v2.5 Milestone (MCP Spec 2026-07-28 v2 Support)

**Execution order:** Phase 112 first and alone → Phases 113, 115, 116 parallelize once the spine lands → Phase 114 sequenced close after 113 (shared stateless-identity/owner-binding pattern) → Phase 117 (needs 113 Client + 114 Tasks) → Phase 118 conformance (validates the union) → Phase 119 docs. Final-spec (2026-07-28) is a checkpoint gating wire-exact values in Phases 112/114.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 112. Version Plumbing Spine | 10/10 | Complete    | 2026-07-23 |
| 113. Stateless HTTP + MRTR | 32/32 | Complete   | 2026-07-27 |
| 113.1 Merge Unblock | 6/6 | Complete | 2026-07-27 |
| 114. Tasks Extension Migration | 20/20 | Plans shipped — awaiting sign-off | 2026-08-01 |
| 115. JSON Schema 2020-12 + Caching Hints | 19/19 | Complete    | 2026-08-02 |
| 116. Auth Hardening SEPs | 16/16 | Complete   | 2026-08-07 |
| 117. Agents, Tester & v1 Severability | 14/14 | Complete    | 2026-08-09 |
| 118. Conformance Against the Official Suite | 10/10 | Complete    | 2026-08-10 |
| 118.1 Close the Nine Conformance Gaps | 14/14 | Complete    | 2026-08-12 |
| 118.2 v1 Client SSE Transport + Log Emitter | 0/12 | Planned — 12 plans in 7 waves | - |
| 119. Documentation — Three Shapes + v2 Migration | 0/TBD | Not started | - |

> **⚠ Phase 113's `Complete` above counts PLANS, not REQUIREMENTS — the phase is HELD, not closed.**
> All 32 plans have SUMMARYs, which is what that column measures. **The publication hold is now
> DISCHARGED, but the phase remains HELD on its OTHER reason** — see the two-reason split in the
> Phase 113 checklist entry above.
>
> **✅ Reason (2), publication, is CLOSED — 2026-08-18, Phase 119 task zero (plan `119-01`).** The
> eleven requirements HTTP-01 … HTTP-08, CLNT-01/02/05 now read **`[x]`**, and
> `113-SPEC-RECHECK.md`'s `## Verdict` reads **`PUBLISHED-CONFIRMED`**. **HTTP-09 was already
> `[x]`, closed on the merits by Phase 113.1** — D-113-R (the quadratic scan over peer-chosen
> input that violated its explicit O(n) clause) is fixed, and D-113-Q (the unbounded `reqwest`
> whole-body read) is bounded, leaving `WHOLE_BODY_ALLOWLIST` EMPTY.
>
> The `[~]` hold had been a **recorded decision, not a default**: plan 113-28's
> `## Third Outcome Policy` in `113-SPEC-RECHECK.md` recorded `hold`, decided by Guy Ernest on
> 2026-07-27, under a trigger that is a **condition** — *"a versioned schema directory exists"* —
> rather than the 2026-07-28 date. That condition is now MET, and **both arms have been run and
> recorded**, which is what the shared landing rule requires: **arm 2** by plan 113.1-04 on
> 2026-07-27 against upstream HEAD `5cc567c3` (predicate byte-identical to its pin, verdict **NO
> DRIFT**, `§ B.6.5`) **and again** by plan `119-01` on 2026-08-18 against the newer HEAD
> `74edef34` with the same NO-DRIFT result and `binary(v2_conformance_pin)` passing 5/5; **arm 1**
> by plan `119-01` on 2026-08-18, landing `PUBLISHED-CONFIRMED` via step-4 row 1. The obligation
> is **DISCHARGED** and no longer rolls forward.
>
> **⚠ Reason (1) is UNTOUCHED — this is why the phase is still HELD.** Phase 119 is a
> documentation phase that changes no `src/` code, so the three open codebase BLOCKERs from
> `113-REVIEW.md` are exactly as they were and still need a gap-closure round. Nothing about this
> discharge should be read as closing Phase 113. Equally, **TASK-01..06 are NOT covered** — they
> are gated by `114-SPEC-RECHECK.md` under the DQ6 *both-repositories* trigger, still
> **`STILL-ABSENT`** (re-measured 2026-08-18: `modelcontextprotocol/ext-tasks` has `draft/` only,
> zero tags, zero releases).
>
> Also open before this branch merges: **D-113-U** — the PR-blocking PMAT complexity gate.
> `write_canonical`'s cog-26 violation (the one this round introduced) was **closed** in
> `58f82368` by splitting the container arms out; canonical bytes and the 64/65 depth boundary
> are byte-identical. The two remaining violations are **CLOSED by Phase 113.1**:
> `handle_post_fast_path` went 30 → **4** and `handle_post_with_middleware` 31 → **4**, each
> paired with a branch-free `*_inner` `?` pipeline measuring **0** (pmat 3.15.0, measured
> per-function at `c9944a65`), so
> `pmat quality-gate --fail-on-violation --checks complexity` passes **locally** with zero
> violations — twenty-one points of margin under the gate's 25.
> *(Corrected 2026-07-28. Plans 113.1-01/05 landed both handlers at 15, which is what this
> block and three other records said; the later cleanup commit `dafc77c5` — not part of any
> 113.1 plan — introduced the wrapper/inner split and changed the figures again. Its own
> message claims "1 (wrapper) + 5 (inner)", which is also wrong. See D-113-U in Phase 113's
> `deferred-items.md` for the measured table and why pmat omits a cognitive-0 function.)*
>
> **The org-required `gate` status check on PR #299 is a separate, still-outstanding matter.** It
> cannot turn green without a human push (D-20 reserves pushing, opening the PR and merging as
> human actions), and two **pre-existing** CI failures unrelated to this phase's three defects
> stand in front of it: `make doc-check` is red on 26 rustdoc errors present at HEAD before Phase
> 113.1 began (recorded as **D-113-W**), and the Purity Gate carries its own known tooling drift.
> Neither was caused by Phase 113.1 and neither is in a merge unblock's scope.

> **✅ Phase 115's `Complete` above counts PLANS *and* REQUIREMENTS — unlike Phases 113 and 114, it
> is genuinely closed.** All 11 plans have SUMMARYs *and* SCHM-01/02/03 are `[x]`, closed on the
> merits rather than held. **The distinction is not a judgement call and it is not inherited:** the
> `[~]` on Phases 113/114 exists because their wire values were read from an UNPUBLISHED schema, and
> D-15 states plainly that *"Phase 115 has NO publication hold and must not inherit a `[~]` booking
> from Phase 114 by habit."* Phase 115's values come from the PUBLISHED
> `modelcontextprotocol/modelcontextprotocol` core schema, vendored at
> `schema/vendored/core-2026-07-28/` @ commit `271ecc9accafdd9b83a3c869fa67c22953b2af80` with both
> digests known in advance and fenced by `tests/vendored_schema_provenance.rs`, so D-15's contingency
> (the Phase-113 HTTP-04 split) never fired. Each booking CITES that artifact plus a named test
> binary and count, so a future reader can re-derive it rather than trust it.
>
> The owner sign-off at `115-10` Task 3 is **answered** — approved by Guy Ernest on 2026-08-01, with
> no corrections — which is the other half of what Phases 113/114 are still missing. The checkpoint
> was returned UNANSWERED by the executing agent rather than self-approved, and
> `git diff --stat 2955d28e..HEAD -- .planning/ROADMAP.md .planning/STATE.md` was verified **EMPTY**
> immediately before the answer: no completion marker existed on disk while the decision was open.
>
> **What Phase 115 does NOT close, and must not be read as closing:** `D-114-S` (nothing watches
> `modelcontextprotocol/ext-tasks` for publication — its `schema/` still carries `draft/` only, so
> Phase 114's D-18 hold stays engaged and TASK-01..06 stay `[~]`) and `D-113-U` (still needs an owner
> before this branch merges). `115-01`'s vendoring closed the CORE half of that trigger and
> `D-114-R` with it; the `ext-tasks` half is untouched.

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

**Branch:** this milestone continues on a rebased `feat/package-remote-capture-show` (254 commits,
31 behind `main`, **zero overlap** with `src/server/`, `src/shared/`, `src/types/` — so it does not
collide with v2.5). That branch already gated its own release tag on an import E2E; this milestone
is finishing what it deliberately left open, not starting fresh.

**Non-goals:** signing keys or PKI in the SDK (decision 1); an ECR client in the CLI (decision 2);
changing `LATEST_PROTOCOL_VERSION` (that is a v2.5 concern and stays pinned); refactoring the
manifest schema for elegance — the schema is expected to churn, so the E2E is the asset, not the API.

- [ ] **Phase 120: Config-Server Packaging** — `pack_server` currently demands `bootstrap: &[u8]`, so a config-only server cannot be expressed. Add vendor media types for the server's own `config.toml` and its OpenAPI spec as layers, and make the binary **dual-mode**: embedded (bootstrap bytes, for a new server or a new version) or referenced (`BinaryRef { digest, media_type }` resolved in the target environment, for a server already deployed there). Both modes are required. Decide and document what is *baked* versus what is a *slot* — the working split is that the spec is baked (it defines the tool surface; change it and it is a different package) while endpoint, credentials and auth mode are slots.
- [ ] **Phase 121: Local Round-Trip E2E** — the regression net, and the piece that needs no backend. Using the London Tube fixture already in `crates/pmcp-openapi-server/tests/fixtures/`: pack in env A → unpack in env B → `detect_deviation` names **exactly** the slots B must fill → fill them → assert **tool-list parity** with A via the existing `parity_replay.rs`. Parity is the property; byte round-tripping is not. This test must survive an arbitrary number of manifest-shape refactors, so assert on behaviour, not on manifest structure.
- [ ] **Phase 122: Attestation Carriage** *(contract-first, parked on backend)* — a layer to hold a pmcp.run-issued attestation and a verification path against pmcp.run's identity. No signing, no crypto dependency. Vendor the attestation contract from the live platform and write the offline blocking contract test; the live half activates when the backend issues attestations.
- [ ] **Phase 123: Export/Import Verbs** *(contract-first, parked on backend)* — `cargo pmcp package pack | unpack | export | import`, resolving environments through `configure`'s existing resolver and reusing the working `deployment/targets/pmcp_run/{graphql,auth}.rs` seam (`PMCP_API_URL`, token cache + TTL) rather than inventing a second API path. `pack`/`unpack` are local and can land immediately; `export`/`import` are contract-first.
- [ ] **Phase 124: Release & Publish Order** — `pmcp-openapi-server` is **absent from CLAUDE.md's publish order** (zero occurrences) and would silently not publish, unlike its siblings `pmcp-sql-server` and `pmcp-workbook-server`. Add it, publish `pmcp-package` 0.2.0 and `cargo-pmcp` 0.19.0, and record the ordering constraint that `pmcp-package` precedes `pmcp-agent` and `cargo-pmcp`.

## Progress — v2.6 Milestone (AI-Package Portability)

**Execution order:** 120 → 121 first and together (no external dependency, and 121 is the regression
net every later refactor leans on) → 122 and 123 in parallel, both contract-first → 124 last.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 120. Config-Server Packaging | 0/TBD | Not started | - |
| 121. Local Round-Trip E2E | 0/TBD | Not started | - |
| 122. Attestation Carriage | 0/TBD | Not started | - |
| 123. Export/Import Verbs | 0/TBD | Not started | - |
| 124. Release & Publish Order | 0/TBD | Not started | - |

> **⚠ Phases 122 and 123 cannot fully close inside this repo.** Both depend on pmcp.run backend
> capabilities — package import and attestation issuance — that were not confirmed as scheduled at
> milestone scoping. They are planned contract-first so the in-repo half is completable and
> verifiable offline. If the backend work is scheduled, promote them from parked to blocking and
> add the live E2E leg.
