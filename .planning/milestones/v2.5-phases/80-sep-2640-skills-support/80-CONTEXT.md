# Phase 80: SEP-2640 Skills Support — CONTEXT

## Phase Boundary

**In scope:**

- Additive `extensions: Option<HashMap<String, Value>>` field on
  `ServerCapabilities` in `src/types/capabilities.rs`, parallel to
  `experimental`. This is GAP #1 from spike 001 — required for wire-correct
  SEP-2640 capability declaration.
- New `Skill` / `SkillReference` / `Skills` types behind a `skills` Cargo
  feature flag on the `pmcp` crate. Lifted near-verbatim from the spike 002
  reference implementation at
  `.planning/spikes/002-skill-ergonomics-pragmatic/src/main.rs`.
- Internal `SkillsHandler` (`ResourceHandler` impl) that serves SKILL.md +
  supporting reference files per the SEP-2640 directory model. List/index
  enumerate SKILL.md entries only — supporting files are readable but
  not enumerated (SEP-2640 §9).
- Internal `ComposedResources` (`ResourceHandler` impl) that URI-prefix-
  routes between skills and any pre-existing `.resources(custom)` handler.
- `SkillPromptHandler` (`PromptHandler` impl) that wraps a `Skill` and
  returns a `GetPromptResult` whose message text is the same content the
  SKILL surface exposes (the dual-surface invariant).
- Three new methods on `ServerCoreBuilder`:
  - `.skill(Skill)` — convenience for single-skill registration.
  - `.skills(Skills)` — registry registration with `Result`-returning
    duplicate-URI rejection.
  - `.bootstrap_skill_and_prompt(Skill, prompt_name)` — registers the
    skill AND a parallel prompt from one `Skill` value (single source of
    truth for both surfaces).
- Auto-set `capabilities.extensions["io.modelcontextprotocol/skills"] = {}`
  and `capabilities.resources` when any skill is registered.
- Paired examples per PMCP convention:
  - `examples/s38_server_skills.rs` — registers hello-world + refunds +
    code-mode skills via the builder; uses
    `.bootstrap_skill_and_prompt(code_mode_skill, "start_code_mode")` for
    the dual-surface demo. Mounts alongside existing `pmcp-code-mode`
    tool registration.
  - `examples/c38_client_skills.rs` — walks both host flows: (a) SEP-2640
    host enumerates skills via `skill://index.json` and reads lazily;
    (b) older host calls `prompts/get start_code_mode` and gets the same
    end-state context eagerly. Same context either way.
- `tests/skills_integration.rs` — in-process server + client; exercises
  all four SEP-2640 endpoints (`resources/list`, `resources/read` for
  SKILL.md, `resources/read` for a reference, `resources/read` for
  `skill://index.json`) AND asserts byte-equality between the SKILL
  surface (concatenated read content) and the PROMPT surface
  (`Skill::as_prompt_text()` / the `prompts/get` response). This
  byte-equality assertion is the load-bearing test that proves the
  dual-surface invariant.

**Out of scope (deferred to v2 — explicit non-goals for this phase):**

- SEP-2640 §4 archive distribution (`application/gzip` + base64 blob).
  Blocked by GAP #2 from spike 001 (`Content::Resource` has no `blob`
  field; the custom `resource_contents_serde::serialize` does not emit
  one). Archive mode is marked **optional** in SEP-2640 §4. Filed as
  a follow-on; don't address here.
- `#[pmcp::skill]` procedural macro that reads SKILL.md from disk at
  compile time and emits a `Skill` constant. Useful but not blocking;
  follow-on spike.
- Tasks (SEP-1686) integration in the same phase. Tasks has its own
  in-repo design doc (`docs/design/tasks-feature-design.md`) and was
  deliberately deferred from the spike session.
- Multi-server skill name disambiguation logic on the *host* side. PMCP
  servers may publish `skill://my-server/code-mode/...` namespaces if
  they want; the host-side disambiguation is a host concern, not an
  SDK concern.

## Implementation Decisions (Non-Negotiable)

Distilled from the spike-findings skill at
`.claude/skills/spike-findings-rust-mcp-sdk/` and the
`.planning/spikes/MANIFEST.md` Requirements section. These are contracts
the plan must honor.

1. **No new traits.** Skills are served via the existing `ResourceHandler`
   trait. Do NOT introduce a parallel `SkillHandler` trait — adds surface
   for zero protocol benefit and breaks composition.

2. **`ServerCapabilities` MUST gain an `extensions` field** (`Option<
   HashMap<String, serde_json::Value>>`) parallel to `experimental`. This
   is GAP #1. Until it lands the capability declaration is wire-incorrect.
   One-line additive change. No breaking change.

3. **Archive distribution is out of scope.** Don't address GAP #2 in this
   phase. Text-mode skills work fully without it.

4. **Skill registration MUST compose with `.resources(custom)`.** Server
   authors must not have to give up their existing resource handler to
   ship skills. URI-prefix routing inside the builder is the validated
   pattern.

5. **`Skills::into_handler()` MUST return `Result` and error on duplicate
   URIs.** Silent overwrite is wrong UX — surfaced as the only meaningful
   paper-cut in spike 002.

6. **Skills MUST support the SEP-2640 directory model.** A skill is a
   SKILL.md plus zero-or-more supporting files
   (`references/schema.graphql`, etc.). Per SEP-2640 §9, supporting files
   are addressable via `resources/read` but MUST NOT be enumerated in
   `resources/list` or the discovery index. The `SkillsHandler` must
   filter explicitly.

7. **Dual-surface rule.** When a skill carries instructions an LLM should
   also be able to load via a prompt (for hosts that don't yet support
   SEP-2640), the prompt body MUST inline the same content — it must NOT
   redirect to the skill URI. A pointer-style prompt body silent-fails
   on SEP-2640-blind hosts because the LLM gets a literal URI string but
   the host has no mechanism to fetch it. PMCP MUST ship
   `bootstrap_skill_and_prompt(skill, prompt_name)` as a single builder
   call that registers both surfaces from one `Skill` value, derived from
   the same data, so they cannot drift. The integration test MUST assert
   byte-equality.

8. **Skills are a general primitive, not a code-mode delivery mechanism.**
   The canonical examples MUST include three tiers (hello-world, refunds,
   code-mode). Hello-world and refunds appear in other SEP-2640 reference
   implementations (TS SDK, gemini-cli, fast-agent, goose, codex) — keeping
   them in the PMCP examples enables cross-SDK developer-experience
   comparison. Code-mode is the *advanced* tier, not the canonical demo.

9. **Feature-gated.** All new types and builder methods are behind a
   `skills` feature flag on the `pmcp` crate. Servers that don't want
   SEP-2640 don't pull the surface.

10. **No new external dependencies.** The reference implementation uses
    only types already in PMCP's dependency graph (`serde_json`,
    `async_trait`, `tokio`). No new crates needed.

## Canonical References

The spike-findings skill is the source of truth for *how* to build this:

- **`.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md`** — top-level
  index of requirements + findings.
- **`.claude/skills/spike-findings-rust-mcp-sdk/references/skills-wire-protocol.md`**
  — implementation blueprint for the protocol-types changes (GAP #1) and
  the wire-format expectations from SEP-2640.
- **`.claude/skills/spike-findings-rust-mcp-sdk/references/skills-dx-layer.md`**
  — implementation blueprint for the `Skill` / `Skills` types, the
  `SkillsHandler` flattening logic, builder integration, and the
  dual-surface pattern.

The spike binaries are runnable references:

- **`.planning/spikes/001-skills-as-resources-mapping/src/main.rs`** —
  in-process demo proving wire-format compliance with SEP-2640 §2, §4,
  §6, §9. Surfaces both protocol-types gaps with assertions.
- **`.planning/spikes/002-skill-ergonomics-pragmatic/src/main.rs`** —
  full reference implementation of every type, handler, and the
  dual-surface byte-equality assertion. STEP 5 in this binary IS the
  invariant the integration test must reproduce.

SEP-2640 spec text (as-of commit `ade2a58`, April 2026):

- PR: https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2640
- Reference implementations (TS SDK, gemini-cli, fast-agent, goose, codex):
  https://github.com/modelcontextprotocol/experimental-ext-skills

Existing PMCP code touched:

- `src/types/capabilities.rs:51` — `ServerCapabilities` (add `extensions`).
- `src/types/resources.rs` — `ResourceInfo`, `ListResourcesResult`,
  `ReadResourceResult` (used as-is, no changes).
- `src/types/content.rs` — `Content` enum + `resource_contents_serde`
  serializer (used as-is, no changes).
- `src/server/builder.rs` — add `.skill()`, `.skills()`,
  `.bootstrap_skill_and_prompt()` methods.
- `src/server/cancellation.rs` — `RequestHandlerExtra` (used as-is).
- `Cargo.toml` — add `skills` feature flag.
- New module: `src/server/skills.rs` (or `src/skills.rs`, planner's call).

## Specific Ideas

- **Module location.** Two reasonable paths: (a) `src/server/skills.rs`
  (next to `builder.rs`, the typical home for builder-adjacent helpers)
  or (b) `src/skills.rs` (top-level, signals "this is a user-facing
  primitive"). The planner should decide and justify; both are defensible.
- **Feature flag default.** `skills` is opt-in by default (not in
  `default-features`). The feature is small and additive, but users who
  don't need SEP-2640 shouldn't pay for the types.
- **`SkillPromptHandler` shape.** The dual-surface prompt handler returns
  a single `PromptMessage` with the concatenated body, or it returns one
  message per file. Spike 002 used a single message for the byte-equality
  check; the real impl could use one-per-file as long as the byte-equal
  invariant still holds after concatenation. Planner's call. Either way,
  the integration test must assert the union.
- **Capability merge.** When `.skills(...)` is called, merge into any
  existing `extensions` map rather than replacing — a future SEP could
  add a second extension and the user may have already set it.
- **Builder ordering.** `.skills(...)` should be callable before OR after
  `.resources(custom)`. The composition logic must handle both directions
  (extending an existing composed handler, or wrapping a later
  registration).

## Deferred Ideas

- `#[pmcp::skill]` macro for compile-time SKILL.md validation. Worth its
  own spike + phase.
- SEP-2640 §4 archive distribution. Requires GAP #2 (add `blob` field
  to `Content::Resource` + emit it from the custom serializer). Separate
  phase if/when archive mode becomes important.
- Skill subscription via `resources/subscribe` for hot-reload during
  development. Not in SEP-2640 spec but a natural follow-on.
- Multi-tenant skill scoping for hosts that aggregate multiple MCP
  servers. Host concern, not SDK concern, but worth surfacing in docs.
- Tasks (SEP-1686) integration. Deferred from the spike session. See
  `.planning/spikes/WRAP-UP-SUMMARY.md` "Deferred for Future Spikes".

## Recommended Plan Shape

(Planner has final say. This is the spike-derived suggestion.)

The work splits naturally into 3 plans:

- **80-01-PLAN.md — Protocol types foundation.** Add `extensions` field
  on `ServerCapabilities` (GAP #1). Add `skills` feature flag to
  `Cargo.toml`. Create the new module skeleton. Land the additive
  protocol change first so subsequent plans can use it. ~1 commit's worth.

- **80-02-PLAN.md — DX layer + builder integration.** Lift `Skill`,
  `SkillReference`, `Skills`, `SkillsHandler`, `ComposedResources`,
  `SkillPromptHandler` from spike 002 into the new module. Wire
  `.skill(...)`, `.skills(...)`, `.bootstrap_skill_and_prompt(...)`
  into `ServerCoreBuilder`. Make `Skills::into_handler()` return
  `Result` with duplicate-URI rejection. ~2-3 commits.

- **80-03-PLAN.md — Examples + integration test.** Ship
  `examples/s38_server_skills.rs` (three-tier skills + code-mode prompt
  fallback) + `examples/c38_client_skills.rs` (both host flows) +
  `tests/skills_integration.rs` (all four SEP-2640 endpoints + the
  byte-equal dual-surface invariant). ~2 commits.

Alternative: 4 plans if examples and integration test are split. Planner
can decide whether to combine or split.
