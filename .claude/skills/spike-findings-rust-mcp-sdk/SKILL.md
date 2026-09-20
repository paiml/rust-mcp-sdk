---
name: spike-findings-rust-mcp-sdk
description: Implementation blueprint from spike experiments — SEP-2640 Skills support (incl. current-draft conformance gaps), the schema-server toolkit lift (config-driven MCP servers for SQL / GraphQL / OpenAPI backends), AND tri-surface skills positioning (skill vs workflow-prompt vs agent — where judgment runs). Requirements, proven patterns, multi-dialect SQL connector trait, two-axis Skills, the dual-surface invariant, and the digest-pin trust model. Auto-loaded during implementation work.
---

<context>
## Project: rust-mcp-sdk

Two complementary additions to the PMCP SDK, validated across two
spike sessions:

**Session 1 (2026-05-12, spikes 001-002):** SEP-2640 Skills support.
Skills are rich, structured agent-workflow instructions discovered and
consumed via existing MCP `resources/*` primitives — no new RPC methods,
but two small additions to PMCP's protocol types are required. The DX
layer provides `Skill` / `SkillReference` / `Skills` types and a
`bootstrap_skill_and_prompt(...)` builder method that registers the same
skill data under both a SEP-2640 surface AND a fallback MCP prompt
surface, so hosts that don't yet support SEP-2640 still get the full
bootstrap context in one round-trip.

**Session 2 (2026-05-17, spikes 003-006):** Schema-server toolkit lift.
The `pmcp-run` service's three "built-in server" core crates (sql,
graphql, openapi) let server authors build a complete MCP server from a
schema file (SQL DDL / OpenAPI YAML / GraphQL SDL) plus a TOML config —
no Rust code required for curated tools. The lift promotes the already-
extracted proto-SDK (`mcp-server-common`, ~2.2k LoC) to a public PMCP
workspace crate, ships per-backend connector crates with multi-dialect
SQL support, AND ships a `pmcp-config-helper` MCP server with Type 2
SEP-2640 authoring Skills (consumed by end-users in their MCP client).

**Session 3 (2026-09-01, spikes 008-011):** Skills positioning.
With the `skills` feature shipped (`src/server/skills.rs`, examples
s44/c10), these spikes re-validated conformance against the CURRENT
SEP-2640 draft (rewritten 2026-08-29: `skills/list` + `skills/get` now
mandatory — the shipped module is non-conformant on the one thing it
declares) and settled how Skills relate to the SDK's two other
instruction-carrying mechanisms: prompts-as-workflows
(`SequentialWorkflow`) and pmcp-agent. Verdict: three points on one
execution-locus axis sharing one content type — the axis is WHERE
JUDGMENT RUNS. Skill is the canonical content primitive the other two
project to (workflow → skill) and consume from (agent ← pinned skill).
Alongside, composing — never instead.

Spike sessions wrapped: 2026-05-12, 2026-05-17, 2026-09-01.
</context>

<requirements>
## Requirements (from MANIFEST.md — non-negotiable)

These are the design contracts the real implementation must honor. Every
reference file below builds on this base.

### From session 1 (Skills SEP-2640):

- **No new traits.** Skills are served via the existing `ResourceHandler`
  trait. Do NOT introduce a parallel `SkillHandler` trait.
- **`ServerCapabilities` must gain an `extensions` field** (parallel to
  `experimental`) to declare SEP-2640 support wire-correctly. One-line
  additive change to `src/types/capabilities.rs:51`.
- **Archive distribution (SEP-2640 §4 `application/gzip` blob) is out of
  scope for v1.** PMCP `Content::Resource` has no `blob` field; the SEP
  marks archive mode optional. Ship text-mode skills first.
- **Skill registration must compose with `.resources(custom)`** —
  URI-prefix routing inside the builder. Server authors must not have to
  give up their existing resource handler to ship skills.
- **`Skills::into_handler()` must error on duplicate URIs** rather than
  silently overwriting.
- **Skills must support the SEP-2640 directory model** (SKILL.md +
  supporting files). Per §9, supporting files are readable via
  `resources/read` but MUST NOT be enumerated in `resources/list` or the
  discovery index.
- **Dual-surface rule.** When a skill carries instructions an LLM should
  also be able to load via a prompt (for hosts that don't yet support
  SEP-2640), the prompt body MUST inline the same content — it must NOT
  redirect to the skill URI. A pointer-style prompt body silent-fails on
  SEP-2640-blind hosts. PMCP must ship a `bootstrap_skill_and_prompt(...)`
  method that registers both surfaces from one `Skill` value so they
  cannot drift.
- **Skills are a general primitive, not a code-mode delivery mechanism.**
  Canonical examples must include three tiers (hello-world, refunds,
  code-mode).

### From session 2 (Schema-server toolkit lift):

- **The shared abstraction already exists.** `mcp-server-common`
  (~2.2k LoC at `pmcp-run/built-in/shared/`) + `pmcp-code-mode` (SDK
  crate) already provide `AuthProvider`, `SecretsProvider`,
  `StaticResourceHandler`, `StaticPromptHandler`, HMAC token machinery,
  `#[derive(CodeMode)]`. All three pmcp-run backend cores already
  consume both. The lift PROMOTES; it does NOT rewrite.
- **No single `SchemaServer<S, C>` trait.** Per-backend executors,
  parameter binding, and policy surface diverge semantically.
  `code_mode.rs` LoC spread is 545 / 767 / 1560 — OpenAPI's 3× weight
  is real (AVP/Cedar, two-tier blocklist, scope binding), not slop.
- **Per-backend connector trait MUST expose `schema_text()`** so the
  code-mode bootstrap prompt body can seed the LLM with the long-tail
  surface in one fetch.
- **Single `SqlConnector` trait + `Dialect` enum** handles all SQL
  backends. 3 methods (`dialect`, `execute`, `schema_text`) + 2 free
  helpers (`translate_placeholders`, `build_code_mode_prompt`) live in
  toolkit core. Per-backend crates own ONLY I/O + their dialect
  declaration. Extending to Oracle / SQL Server / DuckDB is a 3-step
  process that does NOT touch toolkit core.
- **`:name` is the canonical user-facing SQL placeholder syntax.**
  Connectors translate per dialect: Postgres `$1, $2, ...`, MySQL `?`,
  Athena `?`, SQLite identity.
- **Pure-Rust Lambda is the deployment target.** No Docker / no
  testcontainers in spikes or CI. Connector crates use pure-Rust
  drivers (`tokio-postgres`, `sqlx`, `aws-sdk-athena`, `rusqlite`
  bundled). See [[feedback_avoid_docker_pure_rust_lambda]].
- **User-facing surface MUST be ~12 lines of Rust** for Shape C
  (library use); ZERO Rust for Shape A (pure-config binary
  `pmcp-sql-server --config config.toml --schema schema.sql`).
  Asserted in-binary by spike 004 step G.
- **Phase 1 lift scope:** toolkit core + SQL (Postgres / Athena /
  MySQL crates + SQLite feature). GraphQL is Phase 2. OpenAPI is
  Phase 3 (gated by a separate spike resolving AVP/Cedar /
  JS-sandbox / multi-tenant-auth pluggability).
- **Type 1 ↔ Type 2 Skills distinction is non-negotiable in docs.**
  Type 1 = build-time, in `ai-agents/` for coding agents
  (Claude Code, Kiro). Type 2 = runtime, SEP-2640, in
  `pmcp-config-helper` MCP server for end-users via their MCP client.
- **`pmcp::ServerBuilder` needs `tool_arc` + `prompt_arc`** —
  arc-registration methods that `ServerCoreBuilder` already has
  (`src/server/builder.rs:203`). Without them, every config-driven
  toolkit author writes a 20-line delegating wrapper shim.

### From session 3 (Skills positioning, idea `skills-positioning`):

- **SEP-2640 conformance target is the CURRENT draft, not the 2026-05-12
  one the module shipped against.** `skills/list` (MUST), `skills/get`
  (MUST), optional `resources/directory/read`; entries carry verbatim
  frontmatter JSON + `{uri, digest: sha256, size}` manifests;
  `skill://index.json` is nonstandard; archive mode formally dead.
- **Declaring-but-not-implementing is indefensible.** Until the two MUST
  methods land (via the `InternalClientRequest` classifier pattern at
  `src/types/protocol/mod.rs:583` — NO new public `ClientRequest`
  variant, 2.x promise), stop auto-declaring the extension key in
  `set_skills_capabilities`.
- **Workflow↔skill is a projection, not a rival feature.** Ship
  `SequentialWorkflow::as_skill()` (SDK-owned renderer, deterministic,
  byte-equal on re-derivation). Generated body stays a COMPLETE manual
  procedure; the "Server-accelerated alternative" section
  cross-references the workflow prompt but never redirects.
- **Projection owns name slugification** (`refund_flow` → `refund-flow`);
  final URI segment must equal the skill name.
- **Post-hoc judgment is the workflow surface's blind spot.** Guidance
  prose cannot gate server-side execution. Projection-time warning
  required when a side-effecting step carries gate-like guidance. The
  surfaces differ on WHERE JUDGMENT RUNS, not on transport.
- **AgentPackage digest pins ARE SEP-2640 content-bound approval.**
  `[[skills]]` slots `{connector, uri, digest}`, digest REQUIRED; packing
  is the approval ceremony; resolve-time mismatch is revocation (fatal
  ResolveError, pre-loop; never fetch-and-continue). Composition must be
  origin-tagged.
- **System-prompt placement is a privilege decision.** Pinned skill → may
  compose into instructions. Unpinned or `"resources": "dynamic"` → must
  NOT; user-role turn or refusal. Skill-driven supporting-file reads stay
  origin-scoped to the pinning connector (`ToolCall.connector`).
- **Positioning is settled: alongside, composing — never instead.**
  Measured (spike 011): the ineligible order got refunded ONLY by the
  workflow surface. Context cost is NOT monotone — workflow transcript
  outweighed skill body at demo scale (1401 B vs 624 B; agent 82 B).
- **The Skill data model stays as-is.** A fully conforming `skills/list`
  entry is derivable from existing `Skill` fields; fixes are API surface
  (`Skills::entries()`, digest/size computation, name-identity validation
  at build), never a data-model rework.
</requirements>

<findings_index>
## Feature Areas

| Area | Reference | Key Finding |
|------|-----------|-------------|
| Skills Wire Protocol | [`references/skills-wire-protocol.md`](references/skills-wire-protocol.md) | Skills map onto existing `ResourceHandler` without new traits, but PMCP's `ServerCapabilities` lacks `extensions` (GAP #1) and `Content::Resource` lacks `blob` (GAP #2, archive mode only). Fix GAP #1 in the same series as the DX layer. |
| Skills DX Layer + Dual-Surface | [`references/skills-dx-layer.md`](references/skills-dx-layer.md) | `Skill` + `SkillReference` + `Skills` reduce server-author code to ~5 lines per skill. The dual-surface pattern (`Skill::as_prompt_text()` byte-equals concatenated SKILL surface reads) is the load-bearing design decision — prompt body inlines content, never redirects. |
| Schema-Server Architecture | [`references/schema-server-architecture.md`](references/schema-server-architecture.md) | Proto-SDK already extracted at `mcp-server-common` (~2.2k LoC); lift promotes, doesn't rewrite. No single `SchemaServer<S, C>` trait — per-backend executors diverge semantically. Three user-facing shapes: pure-config binary (headline), scaffolded crate, library use (12 lines). |
| SQL Multi-Dialect Connectors | [`references/schema-server-sql-dialects.md`](references/schema-server-sql-dialects.md) | `SqlConnector` 3-method trait + `Dialect` 4-variant enum + 2 free helpers handle Postgres / MySQL / Athena / SQLite. Adding Oracle / SQL Server / DuckDB is a 3-step extension that does NOT touch toolkit core. |
| Type 2 Authoring Skills | [`references/schema-server-authoring-skills.md`](references/schema-server-authoring-skills.md) | `pmcp-config-helper` MCP server ships SEP-2640 Skill bundle for `config.toml` authoring. ~15-line server, content via `include_str!` for Lambda-suitable hermetic binary. End-users consume via their MCP client. |
| SEP-2640 Conformance (current draft) | [`references/sep-2640-conformance.md`](references/sep-2640-conformance.md) | Shipped module is NON-CONFORMANT vs the current draft: auto-declares the extension but lacks the now-mandatory `skills/list` + `skills/get` (a conforming host's first call gets -32601). 7 gaps with file/line fixes; data model sufficient; fix path is the `InternalClientRequest` classifier route. |
| Skills Positioning / Tri-Surface | [`references/skills-positioning-tri-surface.md`](references/skills-positioning-tri-surface.md) | Skill / workflow / agent differ on WHERE JUDGMENT RUNS. Workflow projects to skill (~90 lines, byte-equal); agent consumes digest-pinned skills (~60 lines, zero pmcp-agent changes); measured matrix: only the workflow surface refunded an ineligible order. |

## Source Files

Original spike source files preserved in [`sources/`](sources/):

- [`sources/001-skills-as-resources-mapping/`](sources/001-skills-as-resources-mapping/) — Wire-format compliance demo for SEP-2640 §2, §4, §6, §9.
- [`sources/002-skill-ergonomics-pragmatic/`](sources/002-skill-ergonomics-pragmatic/) — DX reference implementation + dual-surface byte-equality assertion.
- [`sources/003-schema-server-surface-diff/`](sources/003-schema-server-surface-diff/) — Structural diff across the three pmcp-run backend cores.
- [`sources/004-schema-server-thin-slice-sql/`](sources/004-schema-server-thin-slice-sql/) — Inline toolkit slice + SQLite reference + 12-line user surface.
- [`sources/005-multi-dialect-sql-connector/`](sources/005-multi-dialect-sql-connector/) — `SqlConnector` trait + `Dialect` enum + Postgres/MySQL/Athena/SQLite drivers.
- [`sources/006-authoring-skills-server/`](sources/006-authoring-skills-server/) — `pmcp-config-helper` MCP server + SEP-2640 Skill bundle.
- [`sources/008-sep-2640-drift-check/`](sources/008-sep-2640-drift-check/) — Wire-proof drift check vs the current SEP-2640 draft; 7-gap verdict table.
- [`sources/009-workflow-skill-projection/`](sources/009-workflow-skill-projection/) — `project_skill()` renderer + coverage/determinism/registry assertions + the post-hoc judgment transcript.
- [`sources/010-agent-instructions-from-skills/`](sources/010-agent-instructions-from-skills/) — SkillPin + fetch/verify/compose + engine system-prompt byte-equality + tamper refusal.
- [`sources/011-three-mechanisms-one-task/`](sources/011-three-mechanisms-one-task/) — One refund task on three surfaces; shared policy brain; measured decision matrix.

Each source binary is self-contained and runnable:
```bash
cargo run --manifest-path .planning/spikes/00N-name/Cargo.toml
```

## Implementation Order

### Phase 0 (Skills support — from session 1)

1. Land GAP #1 (`extensions` on `ServerCapabilities`) — additive, ~10 LOC.
   *(Already done — spike 008 confirms the declaration is live in
   `set_skills_capabilities`.)*
2. Lift `Skill` / `SkillReference` / `Skills` from spike 002 into `pmcp`
   behind a `skills` feature flag. *(Already done — Phase 80 shipped.)*
3. Add `.skill(...)`, `.skills(...)`, `.bootstrap_skill_and_prompt(...)`
   to `ServerCoreBuilder`. Internal composition over any existing
   `.resources(...)` handler. *(Already done.)*
4. Make `Skills::into_handler()` reject duplicate URIs. *(Already done.)*
5. Add server + client skills examples. *(Already done — shipped as s44/c10.)*
6. Add `tests/skills_integration.rs` asserting all four SEP-2640 endpoints
   AND the byte-equal dual-surface invariant.

### Phase 1 (Toolkit lift — from session 2)

7. **Upstream DX gaps first:** add `tool_arc` + `prompt_arc` to public
   `pmcp::ServerBuilder` (one-line lifts from `ServerCoreBuilder`).
8. **Create `crates/pmcp-server-toolkit/`** — promote `mcp-server-common`
   shape from `pmcp-run/built-in/shared/`. Includes `SchemaServerConfig`,
   `SqlConnector` trait, `Dialect` enum, `translate_placeholders`,
   `build_code_mode_prompt`. SQLite as feature flag for dev/CI.
9. **Create per-backend SQL crates:**
   - `crates/pmcp-toolkit-postgres/` (via `tokio-postgres`)
   - `crates/pmcp-toolkit-athena/` (via `aws-sdk-athena`)
   - `crates/pmcp-toolkit-mysql/` (via `sqlx`)
10. **Create `crates/pmcp-config-helper/`** — Type 2 authoring Skills
    MCP server. Embeds Skill content via `include_str!`. Ship as
    standalone binary AND Lambda-suitable artifact.
11. **Ship pure-config binaries:** `pmcp-sql-server` (one per SQL
    backend crate or unified with `--dialect` flag).
12. **Add `cargo pmcp new --kind sql-server`** scaffolding to
    `cargo-pmcp`. Drops the 12-line `main.rs` + starter Cargo.toml +
    config.toml stub.

### Phase 2 (GraphQL — deferred from session 2)

13. Spike GraphQL connector shape (analogous to spike 005 but for the
    `GraphqlConnector` trait if needed).
14. Create `crates/pmcp-toolkit-graphql/`.

### Phase 3 (OpenAPI — gated on separate spike)

15. **Spike 007** (planned, not yet run): openapi-auth-policy-pluggability —
    can ONE `PolicyEvaluator` trait + auth-passthrough shape serve AVP,
    OPA, Cedar, bespoke RBAC?
16. If yes: create `crates/pmcp-toolkit-openapi/`. If no: defer
    indefinitely; OpenAPI stays at `pmcp-run` as an advanced feature.

### Type 1 Skills content updates (parallel to Phase 1)

17. Update `ai-agents/claude-code/mcp-developer.md` with toolkit content
    (cargo pmcp new flow, custom-handler patterns, dialect selection).
18. Same for `ai-agents/kiro/`.
19. Cross-link Type 1 ↔ Type 2 (Type 1 docs mention `pmcp-config-helper`
    as runtime companion for end-user-facing config curation).

### Session 3 follow-ups (SEP-2640 conformance + positioning)

20. **Land `skills/list` + `skills/get`** via the `InternalClientRequest`
    classifier route (`src/types/protocol/mod.rs:583`) — or stop
    auto-declaring the extension key until they land. CRITICAL: the
    shipped module currently declares a capability it cannot answer.
21. **`Skills::entries()`** — verbatim frontmatter JSON + sha256/size
    manifests, computed at `into_handler()`/build; decide the YAML dep
    (serde_yaml deprecated → serde_yaml_ng / serde-yml / saphyr, or a
    documented flat-frontmatter limit).
22. **Retire (or legacy-gate) `skill://index.json`** in the same change;
    validate name == final URI segment at build; add ≤512-file / ≤16 MiB
    guards.
23. **`SequentialWorkflow::as_skill()`** projection — slugification owned
    by the projection; "Server-accelerated alternative" cross-references,
    never redirects; projection-time warning for gate-like guidance on
    side-effecting steps.
24. **`[[skills]]` digest-pinned slots on AgentPackage** + resolver-side
    fetch/verify/compose in `pmcp-agent::config::resolver`; encode the
    trust rules (pinned→system prompt; unpinned/dynamic→user turn or
    refuse; mismatch→fatal pre-loop; origin-scoped reads via
    `ToolCall.connector`). Grow the pin to the full entry manifest once
    `skills/get` (item 20) lands.
25. **Decision-matrix docs page** for skill vs workflow-prompt vs agent,
    carrying spike 011's measured numbers — mechanism choice is
    deployment, not content.
</findings_index>

<metadata>
## Processed Spikes

### Session 1 (2026-05-12)

- 001-skills-as-resources-mapping (VALIDATED with caveats — two protocol-types gaps surfaced)
- 002-skill-ergonomics-pragmatic (VALIDATED — DX layer + dual-surface invariant proven)

### Session 2 (2026-05-17)

- 003-schema-server-surface-diff (PARTIAL → reframed VALIDATED — proto-SDK already extracted)
- 004-schema-server-thin-slice-sql (VALIDATED — toolkit thin slice runs end-to-end; 12-line user surface; 2 upstream DX gaps surfaced)
- 005-multi-dialect-sql-connector (VALIDATED — 3-method trait + 4-variant Dialect enum handle Postgres/MySQL/Athena/SQLite; 3-step extension protocol for new dialects)
- 006-authoring-skills-server (VALIDATED on first compile — Type 2 SEP-2640 authoring skills via composition of upstream Skill machinery)

### Session 3 (2026-09-01)

- 008-sep-2640-drift-check (VALIDATED — 7 drift gaps vs current draft; declaring-but-not-implementing is the one indefensible state)
- 009-workflow-skill-projection (VALIDATED — faithful deterministic ~90-line projection; post-hoc judgment trap observed live)
- 010-agent-instructions-from-skills (VALIDATED — zero pmcp-agent changes; digest pin == SEP content-bound approval)
- 011-three-mechanisms-one-task (VALIDATED — measured decision matrix; judgment locus is the real axis)

### Not wrappable

- 007-azure-containerapp-deploy — directory holds Azure Container Apps
  deploy/redeploy run logs + a server scaffold from the Phase 80 era; no
  README, never indexed (MANIFEST marks it UNDOCUMENTED). The spike slot
  number is consumed; the content carries no findings to wrap.

## Deferred (not yet spiked)

- **openapi-auth-policy-pluggability** — gates Phase 3 OpenAPI lift. Can ONE `PolicyEvaluator` trait + auth-passthrough shape serve AVP, OPA, Cedar, bespoke RBAC? If no, OpenAPI stays at pmcp-run. (Formerly penciled in as "spike 007"; that number was since consumed by the untracked Azure deploy logs above — this spike remains unnumbered.)
- **tasks-vertical-slice** — validate `tasks-feature-design.md` compiles + runs end-to-end before scaffolding `crates/pmcp-tasks/`.
- **task-retry-expiry-gaps** — pragmatic retry/expiry policy layer above SEP-1686.
- **skills-describing-tasks** — composition of the two primitives in an agent workflow.
- **008 cargo-pmcp-new-from-schema** — scaffolds Shape B (custom-handlers Cargo project). Low risk; depends on Phase 1 toolkit lift being shippable.
- **009 schema-codemode-as-skill** — composition: code-mode bootstrap as SEP-2640 Skill (vs current `/start_code_mode` prompt convention). Connective work after Phase 1 lands.
- `tasks-vertical-slice` — validate `docs/design/tasks-feature-design.md` architecture before scaffolding `crates/pmcp-tasks/`.
- `task-retry-expiry-gaps` — pragmatic retry/expiry policy layer above SEP-1686.
- `skills-describing-tasks` — composition spike for skill + task primitives.
</metadata>
