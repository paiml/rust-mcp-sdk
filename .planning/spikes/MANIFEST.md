# Spike Manifest

## Ideas

### Idea 1 (`skills-sep2640`): SEP-2640 Skills support [VALIDATED — see spikes 001, 002]

Review the experimental MCP Skills extension (SEP-2640, "Skills Over MCP") and
evaluate how the PMCP SDK can support it pragmatically. The PMCP SDK's
"batteries included" stance means: if there is a credible new primitive in the
spec, we want server authors to reach for it via a small, ergonomic API rather
than hand-roll the wire format. The companion question for Tasks (SEP-1686) is
deferred to a later session — the in-repo `docs/design/tasks-feature-design.md`
already covers that primitive in depth, and the user prioritized Skills first
because the spec is genuinely new and the DX shape is unknown.

### Idea 2 (`schema-servers`): Schema-driven configuration-only MCP servers

The `pmcp-run` service ships three "built-in server" core crates
(`mcp-sql-server-core`, `mcp-graphql-server-core`, `mcp-openapi-server-core`)
that let server authors build a complete MCP server from a *schema file*
(OpenAPI / SQL DDL / GraphQL SDL) plus a *configuration file* (TOML). The
config defines the pareto ratio of curated tools (each with a backend-specific
script or query), while the rest of the schema flows through a code-mode layer
for the long tail. The shape is intentional — it's not a 1:1 schema → tools
auto-conversion, it's a curated DX. **Question:** should this capability be
lifted into the PMCP SDK (so anyone outside pmcp.run can build such servers),
exposed via `cargo-pmcp` scaffolding commands, expressed via Rust macros, or
left as a pmcp.run-only advanced feature? Risk-first: prove (or disprove)
that a shared abstraction is real (003), then validate the smallest viable
SDK lift (004).

### Idea 3 (`skills-positioning`): SEP-2640 alongside workflows and agents

Continues Idea 1 with a new question: now that the `skills` feature has shipped
(`src/server/skills.rs`, examples s44/c10), how should SEP-2640 Skills be
POSITIONED against the SDK's two other instruction-carrying mechanisms —
prompts-as-workflows (`SequentialWorkflow`: server executes a deterministic
prefix of tool calls and returns steps + partial results for the LLM to
complete) and pmcp-agent (instructions + LLM config + MCP server list, fully
delegated remote loop)? Working thesis: the three are points on one
execution-locus axis sharing one content type; Skill should become the
canonical content primitive the other two project to / consume from —
alongside, not instead. Spikes 008-011.

**Requirements:**

(Updated as spikes progress. Scoped to this idea.)

- **SEP-2640 conformance target is the CURRENT draft, not the 2026-05-12 one
  the module shipped against.** Spike 008: the draft now defines
  `skills/list` (MUST), `skills/get` (MUST) and optional
  `resources/directory/read`; entries carry verbatim frontmatter JSON +
  complete `{uri, digest: sha256, size}` manifests; `skill://index.json` is
  nonstandard; archive mode is formally dead (Deferred Features appendix).
- **Declaring-but-not-implementing is indefensible.** Until `skills/list` +
  `skills/get` land, either implement them (via the `InternalClientRequest`
  classifier pattern at `src/types/protocol/mod.rs:583` — NO new public
  `ClientRequest` variant, 2.x promise) or stop auto-declaring the extension
  key in `set_skills_capabilities`.
- **Workflow↔skill is a projection, not a rival feature.** Spike 009: ship
  `SequentialWorkflow::as_skill()` (SDK-owned renderer, deterministic,
  byte-equal on re-derivation) so the two surfaces cannot drift. The
  generated body must stay a COMPLETE manual procedure; the
  "Server-accelerated alternative" section cross-references the workflow
  prompt but never redirects.
- **Projection owns name slugification.** Workflow names are not
  agentskills-legal (`refund_flow` → `refund-flow`); the final URI segment
  must equal the skill name.
- **Post-hoc judgment is the workflow surface's blind spot.** Server-side
  execution runs every deterministic step regardless of guidance prose
  (observed live: guidance message immediately followed by the already-
  executed side-effecting tool call). Projection-time warning required when
  a side-effecting step carries gate-like guidance. Positioning language:
  the surfaces differ on WHERE JUDGMENT RUNS, not on transport.
- **AgentPackage digest pins ARE SEP-2640 content-bound approval.** Spike
  010: `[[skills]]` slots `{connector, uri, digest}` with digest REQUIRED —
  the package author is the approving user, packing is the approval
  ceremony, resolve-time digest mismatch is revocation (fatal ResolveError,
  pre-loop; never fetch-and-continue). Composition must be origin-tagged
  (`--- BEGIN SKILL (origin: MCP connector "X", uri …, digest verified) ---`).
- **System-prompt placement is a privilege decision.** Pinned skill → may
  compose into instructions/system prompt. Unpinned or `"resources":
  "dynamic"` → must NOT; user-role turn or refusal. Skill-driven
  supporting-file reads must stay origin-scoped to the pinning connector
  (`ToolCall.connector` is the enforcement point).
- **Positioning is settled: alongside, composing — never instead.** Spike
  011 measured it: same tools + same data, the ineligible order got refunded
  by the workflow surface (deterministic prefix cannot host judgment) and
  declined by the skill and agent surfaces (judgment gates the side effect
  pre-action). Docs guidance: skill = judgment stays with caller; workflow
  prompt = deterministic steps only, judgment-gated side effects OUT of the
  prefix; agent = full delegation, always the cheapest client context.
  Context cost is NOT monotone — the workflow transcript outweighed the
  skill body at demo scale (1401 B vs 624 B; agent 82 B).
- **The Skill data model stays as-is.** Spike 008 proved a fully conforming
  `skills/list` entry is derivable from existing `Skill` fields (name, body,
  references). Fixes are API surface (`Skills::entries()`, digest/size
  computation, name-identity validation at build), never a data-model rework.


## Requirements

(Updated as spikes progress. Non-negotiable for the real build.)

- **Text-mode SKILL.md serving must work without a new `SkillHandler` trait.**
  Spike 001 confirmed that `ResourceHandler` is sufficient. The DX layer in
  spike 002 must be sugar over `ResourceHandler`, not a parallel trait.
- **`ServerCapabilities` needs an `extensions: Option<HashMap<String, Value>>`
  field** (parallel to `experimental`) to declare SEP-2640 support wire-correctly.
  This is a one-line additive change to `src/types/capabilities.rs:51` and is
  independent of any DX work.
- **Archive distribution (SEP-2640 §4, `application/gzip` + base64 blob) is
  out of scope for v1.** PMCP `Content::Resource` has no `blob` field today;
  the SEP marks archive distribution as optional. Ship text-mode skills first.
- **Skill registration must compose with `.resources(...)`** — spike 002
  proved URI-prefix routing inside the builder is the right composition
  pattern. The `Skill` / `Skills` DX must not require the user to give up
  their existing resource handler.
- **`Skills::add()` should error on duplicate URIs** at `.build()` time
  rather than silently overwriting. Spike 002 surfaced silent overwrite as
  the only meaningful UX paper-cut in the DX design.
- **Skills must support the SEP-2640 directory model** — a skill is a
  `SKILL.md` plus zero-or-more supporting files (`references/schema.graphql`,
  `examples.md`, etc.). Per SEP-2640 §9, supporting files are addressable via
  `resources/read` but MUST NOT be enumerated in `resources/list` or the
  discovery index. Spike 002 validated this layout end-to-end.
- **Dual-surface rule.** When a skill carries instructions an LLM should also
  be able to load via a prompt (for hosts that don't yet support SEP-2640),
  the prompt surface MUST inline the same content — it must NOT redirect to
  the skill URI. A pointer-style prompt body silent-fails on SEP-2640-blind
  hosts: the LLM gets a literal string mentioning a URI but the host has no
  mechanism to fetch it. PMCP should ship a `bootstrap_skill_and_prompt(...)`
  builder method that registers both surfaces from one `Skill` value so they
  cannot drift. Spike 002 asserts byte-equality between the surfaces in-binary.
- **Skills are a general primitive, not a code-mode delivery mechanism.** The
  canonical demo includes three tiers (hello-world, refunds, code-mode) so
  the example surface matches other SEP-2640 reference implementations and
  the framing positions code-mode as one valuable application — not the only
  or main use case.

### From Idea 2 (Schema-driven config servers):

- **The shared abstraction is already extracted.** `mcp-server-common`
  (~2.2k LoC at `pmcp-run/built-in/shared/`) plus `pmcp-code-mode` (SDK
  crate) already provide `AuthProvider`, `SecretsProvider`,
  `StaticResourceHandler`, `StaticPromptHandler`, `CODE_MODE_PROMPT_NAME`,
  HMAC token machinery, and the `#[derive(CodeMode)]` macro. All three
  backend cores consume both. Spike 003 confirmed this structurally.
- **No single `SchemaServer<S, C>` trait.** The per-backend executor,
  parameter binding, and policy surface diverge semantically:
  `:name`→`?` vs `$var` vs `{name}`+verb; LIMIT enforcement vs root-field
  allowlist vs HTTP-verb policy + AVP/Cedar. The `code_mode.rs` LoC spread
  (545 / 767 / 1560) reflects real divergence, not implementation slop.
  Any "lift to SDK" must NOT try to unify the per-backend executors behind
  one trait.
- **Promote `mcp-server-common` to `crates/`** (candidate name
  `pmcp-server-toolkit` or `pmcp-builtin`) as the actionable SDK lift.
  Public, stable, on crates.io. Per-backend crates (sql/graphql/openapi)
  stay at pmcp-run but replace their path-dep on `shared/` with a
  versioned crates.io dep on the new toolkit — unblocking independent
  release cadence.
- **`cargo-pmcp new --kind <sql|graphql|openapi>-server` is the right
  developer-facing layer** for this idea. It scaffolds a starter
  `Cargo.toml` + `main.rs` + `config.toml` against the public toolkit
  plus a chosen backend crate. No new runtime abstraction required.
- **`#[pmcp::sql_server]` / `#[pmcp::openapi_server]` proc-macros are
  deferred.** The toolkit being public on crates.io is the prerequisite
  — without it, the macro would expand to types nobody can depend on.
- **Public `ServerBuilder` needs `tool_arc` + `prompt_arc`.** Spike 004
  hit this directly — the user-facing `pmcp::ServerBuilder`
  (`src/server/mod.rs:1741`) only exposes `.tool(name, impl ToolHandler)`
  and `.prompt(name, impl PromptHandler)` (by value), forcing every
  config-driven toolkit author to write a 20-line delegating-wrapper
  shim to share an `Arc<Handler>` between the builder and an
  in-process handler map. `ServerCoreBuilder` (`src/server/builder.rs:203`)
  already has the arc variants; lift them to the public builder.
- **Per-backend connector trait MUST expose `schema_text()`** (or
  equivalent). The code-mode bootstrap prompt body needs a schema
  description to seed the LLM with the long-tail surface; spike 004
  surfaced this naturally when wiring the prompt handler. SQLite impl
  can hand back the seed schema blob; production impls introspect
  `sqlite_master`-style metadata.
- **`Server::handle_request` is private** — external toolkit code
  cannot drive a built `pmcp::Server` in-process. Either expose a
  public `in_process` driver, or document handler-level testing
  (the pattern spike 002 + spike 004 both used) as the recommended
  way to test a config-driven toolkit.

## Spikes

| # | Idea | Name | Type | Validates | Verdict | Tags |
|---|------|------|------|-----------|---------|------|
| 001 | skills-sep2640 | skills-as-resources-mapping | standard | A PMCP server can publish a SEP-2640 Skill via existing `resources/*` primitives, and a representative client can discover + load it with no protocol-extension code. | ✓ VALIDATED (with caveats) | skills, sep-2640, resources, wire-protocol |
| 002 | skills-sep2640 | skill-ergonomics-pragmatic | standard | A PMCP server author can register a skill with a `register_skill(...)` builder (or `#[pmcp::skill]` macro) that mirrors the `register_tool_typed` DX — no hand-rolling URIs, mime types, or naming. | ✓ VALIDATED | skills, dx, batteries-included, builder |
| 003 | schema-servers | schema-server-surface-diff | standard | Given the three pmcp-run built-in core crates (sql/graphql/openapi), when their config schemas, runtime traits, code-mode handlers, and shared deps are structurally diffed, then either a shared SDK-level abstraction is visible with a concrete shape — or shown to be illusory. | ⚠ PARTIAL → reframed ✓ VALIDATED (shared abstraction already extracted at `mcp-server-common` + `pmcp-code-mode`; single per-backend trait NOT viable; lift mcp-server-common to crates/) | schema-server, structural-diff, builtin-servers |
| 004 | schema-servers | schema-server-thin-slice-sql | standard | Given the shared abstraction surfaced by 003, when a minimal SDK-level schema-server primitive is implemented with a SQLite reference connector, then a tiny `config.toml` + `schema.sql` + ~15-line `main.rs` produces a runnable MCP server end-to-end, validating tools/list, tools/call, and the code-mode bootstrap surface. | ✓ VALIDATED (user-facing surface 12 LoC; toolkit slice 346 LoC; SQLite backend 110 LoC; 0 per-tool Rust handlers; 2 DX gaps surfaced upstream) | schema-server, sdk-lift, sqlite, dx |
| 005 | schema-servers | multi-dialect-sql-connector | standard | Given a single `SqlConnector` trait + `Dialect` enum, when the toolkit is driven by ONE `config.toml` with canonical `:name` placeholders against authentic in-process mocks for Postgres ($1,$2 + information_schema), MySQL (? + information_schema), Athena (? + Glue catalog), and a real SQLite, then dialect translation, schema introspection, and dialect-aware code-mode prompt bodies all flow through the trait without per-backend specifics leaking into toolkit core. No Docker / testcontainers (deployment target is pure-Rust Lambda binaries). | ✓ VALIDATED (3-method trait + 4-variant Dialect enum + 2 free helpers cleanly handle Postgres/MySQL/Athena/SQLite; adding Oracle/SQL-Server/DuckDB is a 3-step extension that does NOT touch toolkit core) | schema-server, sql-dialect, postgres, athena, mysql, connector-trait |
| 006 | schema-servers | authoring-skills-server | standard | Given spike 002's upstream `Skill` / `Skills` / `bootstrap_skill_and_prompt` machinery and the `pmcp` `skills` feature flag, when a `pmcp-config-helper` MCP server ships a Skill bundle (root SKILL.md + per-backend references + one worked example) for authoring `config.toml` files, then `resources/list` reports only the root SKILL.md per SEP-2640 §9, `resources/read` serves both the root and supporting references, and `prompts/get` against the dual-surface bootstrap prompt body is byte-equal to the root SKILL.md content (spike 002's invariant holds). | ✓ VALIDATED (passed on first compile — upstream Skills API needs no changes; toolkit gains a 3rd deliverable: `pmcp-config-helper` MCP server with Type 2 SEP-2640 authoring skills) | skills, sep-2640, dual-surface, config-authoring, type-2-skills |
| 007 | (untracked) | azure-containerapp-deploy | standard | (no README — directory holds Azure Container Apps deploy/redeploy run logs + a server scaffold from the Phase 80 era; never indexed) | ⚠ UNDOCUMENTED | azure, deploy |
| 008 | skills-positioning | sep-2640-drift-check | standard | Given the shipped `skills` module (Phase 80), when re-validated against the CURRENT SEP-2640 draft (sep/skills-extension, pushed 2026-08-29), then wire form still complies — or drift gaps are enumerated with file/line fixes. | ✓ VALIDATED (7 gaps: shipped module NON-CONFORMANT vs current draft — auto-declares the extension but lacks the now-mandatory skills/list + skills/get; index.json nonstandard; data model sufficient, gap is API surface) | skills, sep-2640, drift, conformance |
| 009 | skills-positioning | workflow-skill-projection | standard | Given a `SequentialWorkflow`, when projected via a skills-gated adapter, then a derived SKILL.md accurately describes steps/tools/guidance, is discoverable per SEP-2640, and prompt execution is unchanged. | ✓ VALIDATED (~90-line deterministic projection, full coverage asserted; tri-surface invariant holds free; KEY finding: surfaces differ on where judgment runs — guidance cannot gate server-side execution, post-hoc judgment trap observed live) | skills, workflow, projection, tri-surface |
| 010 | skills-positioning | agent-instructions-from-skills | standard | Given a pmcp-agent config referencing a skill URI on one of its servers, when resolved, then instructions compose base + fetched skill body (digest-verified, origin-tagged) and the engine's system prompt provably contains it. | ✓ VALIDATED (zero pmcp-agent changes; ~60-line resolve layer; tamper refused pre-loop; KEY finding: AgentPackage digest pin == SEP content-bound approval, same mechanism two sides — packaging territory the WG scoped out, pmcp-package already ahead) | skills, pmcp-agent, pmcp-package, trust-boundary |
| 011 | skills-positioning | three-mechanisms-one-task | standard | Given one domain task (refund flow), when implemented as skill vs workflow-prompt vs agent with the same tools and decision rules, then a harness measures round-trips, context weight, and where judgment ran — producing a decision matrix for SDK docs. | ✓ VALIDATED (measured matrix; headline: ineligible order refunded ONLY by the workflow surface — judgment locus is the real axis; surfaces compose: workflow→skill (009), agent←skill (010), same decide() at two loci) | skills, workflow, pmcp-agent, decision-matrix |

**Deferred (not in this session):**

- `tasks-vertical-slice` — validate the existing `tasks-feature-design.md`
  architecture compiles + runs end-to-end before scaffolding `crates/pmcp-tasks/`.
- `task-retry-expiry-gaps` — pragmatic retry/expiry policy layer above SEP-1686.
- `skills-describing-tasks` — composition of the two primitives in an agent
  workflow.
