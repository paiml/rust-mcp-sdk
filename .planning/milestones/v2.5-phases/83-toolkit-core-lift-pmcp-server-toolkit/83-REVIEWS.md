---
phase: 83
reviewers: [gemini, codex]
reviewed_at: 2026-05-18T13:04:18Z
plans_reviewed:
  - 83-01-PLAN.md
  - 83-02-PLAN.md
  - 83-03-PLAN.md
  - 83-04-PLAN.md
  - 83-05-PLAN.md
  - 83-06-PLAN.md
  - 83-07-PLAN.md
  - 83-08-PLAN.md
  - 83-09-PLAN.md
---

# Cross-AI Plan Review — Phase 83: Toolkit Core Lift (`pmcp-server-toolkit`)

Two external AI CLIs (Gemini 0.38.2 + Codex 0.129.0) independently reviewed the 9 plans plus CONTEXT.md, RESEARCH.md, PATTERNS.md, and the relevant REQUIREMENTS.md slice. Each was given the same 437KB prompt and asked for Summary / Strengths / Concerns / Suggestions / Risk Assessment.

---

## Gemini Review

# Cross-AI Plan Review: Phase 83 Toolkit Core Lift

## 1. Summary
Phase 83 is a highly detailed, well-structured promotion of the `mcp-server-common` proto-SDK into a public `pmcp-server-toolkit` crate. The 9-plan execution strategy effectively decomposes a high-risk cross-repo lift into safe, verifiable steps. The heavy reliance on TDD (especially in Plan 05 for tool synthesis), strict parsing (`deny_unknown_fields`), and explicit feature-gating demonstrates a mature approach to API design. However, the phase carries residual execution risk around cross-repo dependencies (fetching source files via absolute paths) and the exact instantiation mechanics of the `CodeExecutor`, which are left as a `todo!()` for the agent to resolve dynamically.

## 2. Strengths
- **Granular Wave Structure:** The 6-wave dependency graph correctly isolates scaffolding (Wave 1), verbatim lifts and config parsing (Wave 2), net-new synthesis and wiring (Waves 3-4), and capstone verifications (Waves 5-6).
- **Strict Parsing Discipline:** Enforcing `#[serde(deny_unknown_fields)]` in Plan 04 while maintaining the `REF-01` superset via explicit field enumeration prevents security bypasses via typos (e.g., `auto_aprove_levels`).
- **Secrecy Enforcement:** Plan 02's strict requirement to refactor `SecretsProvider::get` to return a `TokenSecret` (with negative trait assertions) effectively neutralizes the risk of secret leakage in the lifted code.
- **In-Toolkit Smoke Test:** Plan 08's `backend_core_smoke.rs` is an elegant workaround to validate the cross-repo API contract without requiring a cross-repo CI setup during this phase.
- **Agent Grounding:** The plans heavily use `<read_first>` gates and `key_links` to ground the agent in the workspace before executing, reducing hallucination risk.

## 3. Concerns

- **[HIGH] Fragile External Path Dependencies (Plans 01, 02, 03, 04, 09):** The plans rely on hardcoded absolute paths to a sibling repo (e.g., `~/Development/mcp/sdk/pmcp-run/...`). If the agent environment lacks this exact path or read permissions, the plans explicitly instruct the agent to halt and demand operator handoff. This breaks the autonomous flow immediately in Waves 1 and 2.
- **[HIGH] Blind Spots in `CodeExecutor` Instantiation (Plan 06, Tasks 1 & 2):** Task 1 leaves a `todo!()` for instantiating the `CodeExecutor`, instructing the agent to "read pmcp-code-mode source to determine which" constructor to use. If the constructor requires generic bounds, lifetimes, or trait objects not easily inferred, the agent may thrash or fail to compile the wiring.
- **[MEDIUM] API Ergonomics Drift in Examples (Plan 08):** In Plan 08's `e01_toolkit_minimal.rs`, the imports are `use pmcp_server_toolkit::{auth::StaticAuthProvider, config::ServerConfig, resources::StaticResourceHandler, ServerBuilderExt};`. This contradicts the "single dep, crate-root re-exports" promise (D-15). If `StaticAuthProvider` is re-exported at the root (Plan 02), the example should just be `use pmcp_server_toolkit::{StaticAuthProvider, ServerConfig, ...};`.
- **[MEDIUM] Premature `SqlConnector` Trait Ossification (Plan 07):** Locking the `SqlConnector` trait to just `dialect`, `execute`, and `schema_text` based on spike 005 risks boxing in Phase 84. Passing `&str` and returning `Vec<Value>` precludes efficient streaming of large result sets or transaction management, which are common enterprise SQL requirements.
- **[LOW] Manual `CodeModeSection` Alias Mapping (Plan 06):** Plan 06 manually maps unprefixed config fields to `sql_`-prefixed `CodeModeConfig` fields. If `pmcp-code-mode` adds new fields in the future, this manual mapping must be updated, creating a maintenance burden.

## 4. Suggestions
- **Provide a Fallback for External Files:** Instead of halting on missing sibling-repo paths, embed a minimal fallback version of the `mcp-server-common` files in a `.planning/fixtures/` directory, or instruct the agent to use `pmcp`'s own mock implementations as a fallback to ensure the phase can complete autonomously.
- **Pre-resolve the `CodeExecutor` Constructor:** Update Plan 06 to provide the exact `pmcp_code_mode` constructor signature rather than leaving a `todo!()`. If it's `DefaultCodeExecutor::new`, specify that directly to prevent agent hallucination.
- **Fix the Shape C Example Imports:** Update `e01_toolkit_minimal.rs` in Plan 08 to strictly use the root-level re-exports (e.g., `use pmcp_server_toolkit::{StaticAuthProvider, ServerConfig, StaticResourceHandler, ServerBuilderExt};`) to prove the D-15 ergonomics decision is working as intended.
- **Add Streaming/Transaction Notes to `SqlConnector`:** In Plan 07, add a comment or `todo` indicating that `SqlConnector` may need extension in Phase 84 to support `futures::Stream` or transactions, ensuring the trait isn't considered permanently locked by downstream consumers.
- **Refine `SynthesizedToolHandler::handle`:** In Plan 05, the placeholder execution returns a text error. Ensure the error is clearly formatted as a `CallToolResult` with `is_error: true` so MCP clients don't parse it as a successful tool invocation.

## 5. Risk Assessment
**Overall Risk Level: MEDIUM**

*Justification:* The architectural design, security controls, and testing strategies are exceptional (LOW risk). However, the execution risk is elevated to MEDIUM due to the hard dependency on an external sibling repository at an absolute file path. If the CI/agent environment does not have the `pmcp-run` repository checked out at `~/Development/mcp/sdk/pmcp-run/`, the entire Phase 83 autonomous execution will halt. Furthermore, the `CodeExecutor` wiring `todo!()` injects unnecessary ambiguity into a critical security component. Fixing the external dependency pathing and pre-resolving the `todo!()` would lower the overall risk to LOW.

---

## Codex Review

**Summary**

The phase is well-researched and the plan set is unusually explicit, but I would not treat it as execution-ready without tightening three areas: `pmcp-code-mode` integration, `SqlConnector`/placeholder semantics, and the cross-repo `pmcp-run` migration proof. The scaffold/config/tool-synthesis portions are strong. The risky parts are where the plans assume APIs or runtime behavior that may not exist yet, especially Plan 06's generic `CodeExecutor` construction and Plan 07's SQL parameter translation.

**Strengths**

- Strong traceability from requirements to plans: TKIT-01..10 and TEST-02/03 are mostly mapped to concrete files, tests, and artifacts.
- Good use of locked decisions: D-05 workspace-version deps, D-13 strict config parsing, D-14 slim feature matrix, D-16 HMAC re-export-only, and D-17 test coverage are consistently referenced.
- Plan 04's reference-fixture parse test is a solid REF-01 anchor, assuming the fixtures are real snapshots.
- Plan 05 correctly protects the Phase 82 `tool_arc` invariant by requiring `SynthesizedToolHandler::metadata() -> Some(ToolInfo)`.
- The phase graph is mostly sensible: scaffold/config first, lift and net-new modules in parallel, builder capstone, publish/fuzz/contract finalization.
- The docs artifacts in Plan 09 are useful: shim handoff and migration guide reduce operational ambiguity.

**Concerns**

- **HIGH — Plan 06 / `src/code_mode.rs`: `executor_from_config` may not be implementable as written.** The plan assumes the toolkit can build a complete `Box<dyn CodeExecutor>` from config alone. Real execution usually needs a backend executor, connector, DB client, or closure. If `pmcp-code-mode` only validates code and does not provide a concrete generic executor, "zero per-server Rust glue" is not satisfied.

- **HIGH — Plan 06 Task 2 / `register_code_mode_tools`: tool registration is underspecified.** The plan says to prefer existing pmcp-code-mode handlers if they exist, otherwise synthesize `validate_code` / `execute_code`. That is a major behavioral surface, not a minor implementation detail. It needs a pre-plan spike or explicit API contract.

- **HIGH — Plan 07 / `sql::translate_placeholders`: translation loses binding order.** Returning only `String` is insufficient. Postgres/MySQL/Athena need ordered bind values. Repeated placeholders like `:id OR :id` also need defined semantics. Consider returning a struct: `{ sql, ordered_params }`.

- **HIGH — Plan 07 / `SqlConnector` may box in Phase 84.** `execute(&self, sql, params) -> Vec<Value>` commits to row shape, error shape, placeholder ownership, async behavior, and parameter model before real connector impls exist. This should be validated against at least one tiny Postgres/MySQL/Athena mock or spike before freezing public API.

- **HIGH — Plan 08 / TKIT-08 proxy is not fully credible for "all three backend cores."** `tests/backend_core_smoke.rs` uses one SQL fixture and a synthetic construction surface. It does not prove GraphQL/OpenAPI cores compile, feature gates match, old import names exist, or dropped DDB/OpenAPI features are harmless.

- **MEDIUM — Plan 02 / root API does not clearly re-export `AuthProvider`.** ROADMAP SC-1 says developers can import `AuthProvider` from the toolkit. Plan 02 suggests `pub use auth::{AuthProvider as _, StaticAuthProvider}`, which is not a useful public re-export. Use `pub use pmcp::server::auth::AuthProvider;` or an exact verified path.

- **MEDIUM — Plan 02 / `SecretsProvider::get -> TokenSecret` conflicts with optional `code-mode`.** If `TokenSecret` comes from `pmcp-code-mode`, then `--no-default-features` either breaks secrets or creates a different local `TokenSecret` type. That is a public API trap. Define one stable toolkit secret type, or make `pmcp-code-mode`'s token type available independently.

- **MEDIUM — Plan 02 negative trait tests are not real tests.** Commented-out compile-fail assertions do not enforce "not Debug / not Clone / not Serialize." Use `trybuild` compile-fail tests or avoid claiming enforcement.

- **MEDIUM — Plan 04 / `ServerConfig` may parse invalid required sections too permissively.** Heavy use of `Default` on required-looking fields (`server.name`, `server.version`, tool names) catches typos but not missing required values. Add a `validate()` pass or make truly required fields non-default.

- **MEDIUM — Plan 06 security: `token_secret` inline TOML fallback is risky.** A warning is not much protection. Prefer `env:` only by default, with inline literals behind `allow_inline_token_secret_for_dev` or test-only helpers.

- **MEDIUM — Plan 06 dependency metadata is incomplete.** Plan 06 depends only on Plan 04, but it references `secrets::EnvSecrets` / `TokenSecret` behavior from Plan 02. Add `depends_on: [02, 04]`.

- **MEDIUM — Plan 08 builder methods panic.** `tools_from_config` and `code_mode_from_config` using `.expect()` makes common-path public API crash on missing env/config. Keep those if desired, but add `try_tools_from_config` and `try_code_mode_from_config`.

- **LOW — Plan 01/09 publish packaging excludes `tests/` and `fuzz/`.** That is fine for tarball size, but then integration tests are not shipped to consumers. If that is intentional, document it. If docs.rs examples rely on fixtures, this will break.

- **LOW — Plan 05 placeholder handler can mislead.** `SynthesizedToolHandler::handle` returns "not wired" until Phase 84. That is acceptable for metadata synthesis, but the README and migration guide must not imply config-defined SQL tools execute in Phase 83.

**Suggestions**

- Add a preflight task before Plan 06: verify exact `pmcp-code-mode` construction/registration APIs and write a short `CODE_MODE_API_NOTES.md`. Do not leave this to executor discovery.
- Change `executor_from_config` into two layers: `validation_pipeline_from_config(&ServerConfig)` and `code_mode_tools_from_executor(executor, config)`. Require backend execution injection if needed.
- Redesign placeholder translation to return translated SQL plus ordered parameter names/values. Test repeated params, `::` casts, string literals, comments, and no-placeholder identity.
- Add a tiny "external compatibility fixture" for TKIT-08: a local shim crate named `mcp-server-common` with old-style imports that depends on `pmcp-server-toolkit`. This is a better proxy than only building a server from `open-images`.
- Make root re-exports explicit in Plan 08/09: `AuthProvider`, `SecretsProvider`, `ServerConfig`, `StaticResourceHandler`, `StaticPromptHandler`, `SqlConnector`, `Dialect`, `ServerBuilderExt`.
- Add `ServerConfig::validate()` and run it after `from_toml` in key paths, or expose `from_toml_strict_validated`.
- Replace inline token secrets with env-only production behavior. If inline is kept, gate it with a clearly named dev/test option.
- Split Plan 06 if needed: one plan for re-exports/config mapping, one for actual tool registration. It is currently the highest-risk plan.

**Risk Assessment**

Overall risk: **HIGH**.

The scaffolding, config parsing, lifted static handlers, and `ToolInfo` synthesizer are medium-risk and well planned. The high risk comes from committing public API around two not-yet-proven integration contracts: generic code-mode execution and SQL connector semantics. The TKIT-08 smoke test also does not fully prove the real cross-repo migration. With a code-mode API preflight, a corrected SQL parameter model, and a stronger compatibility proxy for `mcp-server-common`, this drops to **MEDIUM**.

---

## Consensus Summary

Both reviewers agree the phase is well-researched, traceable, and disciplined on locked-decision adherence. They split on overall risk (Gemini: MEDIUM, Codex: HIGH), and the divergence is informative: Gemini weighted execution/environment risk (external sibling-repo path dependency); Codex weighted design risk on API surfaces that get frozen in 0.1.0 (Plan 06 code-mode wiring, Plan 07 SqlConnector trait + placeholder translation). The intersection is small but high-signal.

### Agreed Strengths (mentioned by both reviewers)

- **Requirement traceability** — every TKIT-XX and TEST-XX maps to a concrete plan/task/file (Gemini "Agent Grounding"; Codex "Strong traceability from requirements to plans").
- **Strict-parse discipline + REF-01 superset** — `deny_unknown_fields` + explicit field enumeration is the right pattern (Gemini "Strict Parsing Discipline"; Codex "D-13 strict config parsing").
- **Secrecy / TokenSecret refactor in Plan 02** — both note the negative-trait stance is the right direction (with Codex flagging the test mechanism itself — see Divergent Views).
- **Phase 82 `tool_arc` substrate is used correctly** in Plan 05 (Codex explicit; Gemini implicit via "API design maturity").
- **Wave structure is sensible** (Gemini "Granular Wave Structure"; Codex "The phase graph is mostly sensible").

### Agreed Concerns (raised by both reviewers — highest priority)

1. **[HIGH] Plan 06 `CodeExecutor` instantiation is underspecified.**
   - Gemini: "Blind Spots in `CodeExecutor` Instantiation … may thrash or fail to compile."
   - Codex: "`executor_from_config` may not be implementable as written … if `pmcp-code-mode` only validates code, 'zero per-server Rust glue' is not satisfied."
   - **Action:** Pre-resolve `pmcp-code-mode` construction/registration APIs before Plan 06. Replace `todo!()` with the verified constructor signature. Consider Codex's two-layer split (`validation_pipeline_from_config` + `code_mode_tools_from_executor`).

2. **[HIGH/MEDIUM] Plan 07 `SqlConnector` trait risks ossifying public API before connectors exist.**
   - Gemini: "Premature `SqlConnector` Trait Ossification … precludes efficient streaming … transaction management."
   - Codex: "may box in Phase 84 … commits to row shape, error shape, placeholder ownership, async behavior, and parameter model before real connector impls exist."
   - **Action:** Either downgrade the trait to a minimal `dialect` + `schema_text` shape and defer execute()/translate to Phase 84, OR validate against a tiny Postgres/MySQL/Athena mock now. Codex's `{ sql, ordered_params }` return type for placeholder translation is required either way — `String`-only translation drops bind ordering.

3. **[MEDIUM] Plan 08 example imports contradict the D-15 "single dep / crate-root re-exports" promise.**
   - Gemini: "API Ergonomics Drift in Examples … example should just be `use pmcp_server_toolkit::{StaticAuthProvider, ...}`."
   - Codex: "root API does not clearly re-export `AuthProvider` … 'pub use auth::{AuthProvider as _, StaticAuthProvider}' is not a useful public re-export."
   - **Action:** Fix Plan 02 to `pub use pmcp::server::auth::AuthProvider;` (or the verified pmcp path) at the crate root, and update Plan 08's `e01_toolkit_minimal.rs` to import from the crate root only. This is the headline DX promise of the phase — if it's broken in the example, it's broken everywhere.

### Divergent Views (worth investigating)

1. **Overall risk: MEDIUM (Gemini) vs HIGH (Codex).** The gap is whether you weight environment/setup risk (sibling repo at absolute path) or design risk (frozen-in-0.1.0 public API). Both are real; the latter is harder to back out after a publish.

2. **External sibling-repo path dependency (Gemini HIGH, Codex didn't dwell).** Gemini wants a `.planning/fixtures/` fallback or pmcp mocks; Codex implicitly accepts the operator-handoff escape (`user_setup` blocks in plans). If the planner runs in CI without `~/Development/mcp/sdk/pmcp-run/`, Gemini is right that execution halts immediately.

3. **Plan 02 negative-trait-bound assertions (only Codex flagged).** Codex: "Commented-out compile-fail assertions do not enforce 'not Debug / not Clone / not Serialize.' Use `trybuild` compile-fail tests or avoid claiming enforcement." The TokenSecret threat model claims this protection — if assertions are commented-out, the protection is theatre.

4. **Plan 02 `SecretsProvider::get -> TokenSecret` cross-crate type leak (only Codex flagged).** With `code-mode` as a default but optional feature, `--no-default-features` either breaks secrets or forks the type. Likely needs a stable toolkit-owned secret type or a re-export through a non-feature-gated module.

5. **Plan 06 missing `depends_on: [02]` (only Codex flagged).** Plan 06 references `secrets::EnvSecrets` / `TokenSecret` behaviors landed in Plan 02 but only declares dep on 04. The plan-checker missed this in iteration 2.

6. **Plan 08 `.expect()` panics in `tools_from_config` / `code_mode_from_config` (only Codex flagged).** Public API panics on common-path errors. Add `try_*` variants.

7. **TKIT-08 smoke test credibility (CONTEXT.md D-03 alternate).** Gemini calls it "elegant"; Codex says it "is not fully credible for 'all three backend cores'." Codex's counter-proposal — a tiny local shim crate named `mcp-server-common` with old-style imports — would actually prove the migration shape compiles.

8. **`ServerConfig::validate()` for required-field semantics (only Codex flagged).** Default impls on `server.name` / `server.version` / tool names hide missing-required-value bugs behind silent empty strings. Codex wants `from_toml_strict_validated`.

9. **`translate_placeholders` return type (only Codex flagged).** `&str -> String` is insufficient for any dialect needing positional binds (Postgres `$1`, MySQL `?`, Athena `?`). Must return ordered param list.

### Recommended Next Action

Three of the agreed concerns translate into concrete revisions before `/gsd:execute-phase 83`:

- **Plan 06 preflight task (CodeExecutor API verification).** Add a Wave 1 or Wave 2 task that reads `crates/pmcp-code-mode/src/code_executor.rs` and `lib.rs`, writes `CODE_MODE_API_NOTES.md` to the phase dir capturing the exact construction + registration signatures, then references that doc from Plan 06 instead of `todo!()`.
- **Plan 02 crate-root re-export fix.** Change `pub use auth::{AuthProvider as _, StaticAuthProvider}` to the actual `AuthProvider` re-export path (`pub use pmcp::server::auth::AuthProvider;` or equivalent). Update Plan 08's example imports to match.
- **Plan 07 `SqlConnector` minimization OR validation spike.** Either reduce the trait to `dialect()` + `schema_text()` only (deferring `execute` + placeholder translation to Phase 84 after CONN-04 connector impls exist), OR add a tiny `MockSqlConnector` over Postgres/MySQL/Athena placeholder shapes and validate the `{ sql, ordered_params }` translation contract now.

Codex's recommended split of Plan 06 into two plans (re-export/config-mapping vs tool registration) is worth considering — Plan 06 is currently the highest-risk plan.

To incorporate this feedback into the plans:

```
/gsd:plan-phase 83 --reviews
```

The `--reviews` flag instructs the planner to read this file and apply targeted updates without replanning from scratch.
