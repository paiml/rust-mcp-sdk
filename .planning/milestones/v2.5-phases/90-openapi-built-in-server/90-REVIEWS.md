---
phase: 90
reviewers: [gemini, codex]
reviewed_at: 2026-05-29T15:55:47Z
plans_reviewed: [90-01-PLAN.md, 90-02-PLAN.md, 90-03-PLAN.md, 90-04-PLAN.md, 90-05-PLAN.md, 90-06-PLAN.md, 90-07-PLAN.md, 90-08-PLAN.md, 90-09-PLAN.md]
---

# Cross-AI Plan Review — Phase 90

> Two independent external AI CLIs reviewed all 9 plans. Gemini rated overall risk **LOW**;
> Codex rated overall risk **MEDIUM-HIGH** and surfaced three HIGH integration concerns Gemini
> did not raise. The divergence is the signal — see the Consensus Summary at the bottom.

## Gemini Review

# Cross-AI Plan Review: Phase 90 OpenAPI Built-In Server

## Summary
The execution plans for Phase 90 are exceptionally well-structured, tightly scoped, and rigorously aligned with the architectural decisions (D-01 to D-06). The plans correctly identify and leverage the existing pure-Rust AST `PlanCompiler` / `PlanExecutor` instead of introducing a second JS engine (Boa), ensuring semantic parity between Code Mode and curated script tools. Feature gating is handled flawlessly via the `openapi-code-mode` umbrella feature, keeping single-call builds light without SWC overhead. The 6-wave dependency graph is sound, and security constraints (credential redaction, execution bounds) are consistently enforced across the pipeline. 

## Strengths
* **Engine Unification (D-02):** Flawlessly maps out the reuse of the SDK's existing `PlanCompiler` and `PlanExecutor`, avoiding the trap of implementing a second JS engine. The engine-parity wiremock test in Plan 05 is a robust way to enforce this invariant.
* **Feature Gating Precision:** The distinction between `http`, `code-mode`, and the `openapi-code-mode` umbrella feature is expertly managed. It guarantees that curated-only (single-call) servers avoid the SWC parser bloat.
* **Security & Redaction Discipline:** Consistently mandates `test_*_display_does_not_echo_secret` across `HttpConnectorError`, `DispatchError`, and `ExecutionError`. This comprehensively addresses Pitfall 5 (credential leakage).
* **Non-Breaking SQL Generalization:** Plan 04 elegantly widens `code_mode_tools_from_executor` to accept `Arc<dyn CodeExecutor>` and parameterizes the validation flavor, achieving backend-agnostic wiring without regressing the existing SQL path.
* **Incremental Synthesis (OAPI-02):** Splitting single-call synthesis (Plan 03) and script tool synthesis (Plan 05) ensures clean testability and respects the dependency order of the `HttpCodeExecutor` seam.

## Concerns
* **[LOW] Transient Error State in Plan 03:** Plan 03 instructs the synthesizer to return an explicit `ToolkitError` for script tools until Plan 05 lands. While perfectly safe within the context of a sequential CI build, it leaves the `main` branch in a transiently unsupported state for script tools if the pipeline were to halt mid-phase. 
* **[LOW] Trait Object Coercion in Plan 04:** When updating the SQL callers to pass `Arc::new(sql_exec) as Arc<dyn CodeExecutor>`, Rust's compiler might require explicit coercion depending on trait bounds. The implementer will need to ensure `CodeExecutor` is object-safe (which it appears to be, given it uses `#[async_trait]`).
* **[LOW] Base URL Normalization:** Both `HttpClient` and `HttpCodeExecutor` manually concatenate `base_url` and `path` to avoid dropping API Gateway prefixes (Pitfall 2). Ensuring that trailing and leading slashes are consistently trimmed in both places is critical to avoid `//` or malformed URLs. The plan mentions `trim_end_matches('/')` + `trim_start_matches('/')`, which mitigates this.

## Suggestions
* **URL Concat Helper:** Consider extracting the safe URL concatenation logic (Pitfall 2) into a tiny shared helper function inside `toolkit::http` rather than duplicating the `trim_end_matches('/')` logic in both `HttpClient::execute` (Plan 01/03) and `HttpCodeExecutor::execute_request` (Plan 04).
* **Script Tool Bounds Override:** While Plan 05 correctly defaults to the shared `[code_mode]` `ExecutionConfig` for script tools, consider adding a comment in the code acknowledging that per-tool `ExecutionConfig` overrides (e.g., `[[tools.execution_config]]`) could be a future additive enhancement if script tools need different bounds than the LLM Code Mode.
* **Validation Flavor Enum:** In Plan 04, favor a strict `pub enum ValidationFlavor { Sql, OpenApi }` over a `&str` parameter for `code_mode_tools_from_executor`. This provides stronger compile-time guarantees and avoids typos when routing to `JavaScriptValidator`.

## Risk Assessment
**Overall Risk: LOW**

The architectural constraints have been thoroughly de-risked during the research phase. The most complex integration point—sharing the AST JS engine between two distinct handler paths—has been cleanly mapped to the existing `pmcp-code-mode` abstractions. The plans exhibit a high degree of defensive programming (redaction checks, strict TOML parsing, explicit HTTP path concatenation) and the test strategy (wiremock + parity harness) is comprehensive. Execution should be straightforward.

---

## Codex Review

## Overall Summary

The plan set is strong and unusually well grounded in prior phases, reference code, and explicit decisions. It should achieve the phase goal if the feature gates and OAPI-10 refactor are tightened. The biggest risks are not scope coverage; they are integration correctness: Wave 1 has a hidden dependency between Plans 01 and 02, Plan 04 contradicts the `openapi-code-mode` feature-gating rule, and `oauth_passthrough` cannot work with the current startup-created auth provider / `HttpExecutor` shape unless per-request auth context is explicitly designed.

## Cross-Plan Concerns

- **HIGH: `oauth_passthrough` is architecturally under-specified.** The plans create auth providers at dispatch/startup, but passthrough needs the incoming MCP auth token per request. `HttpConnector::execute(operation, args)` and `pmcp_code_mode::HttpExecutor::execute_request(method, path, body)` do not receive `RequestHandlerExtra` or `AuthContext`. Shipping `oauth_passthrough` without a per-request token path will produce a configured variant that cannot work correctly.

- **HIGH: Plan 04 feature gates conflict with Plan 01/05.** Plan 01 correctly defines `openapi-code-mode = ["http", "code-mode", "pmcp-code-mode/js-runtime"]`, and Plan 05 gates script tools on it. Plan 04 gates `HttpCodeExecutor` under `all(feature="http", feature="code-mode")` and tests with `--features "http code-mode"`, which likely omits `pmcp-code-mode/js-runtime` and the JS/OpenAPI executor exports.

- **HIGH: Plan 02 is not actually independent of Plan 01.** It references `crate::http::auth::AuthConfig` and `crate::http::client::HttpConfig`, which Plan 01 creates. If the wave executes plans in parallel, Plan 02 cannot compile until Plan 01 lands.

- **MEDIUM: “five auth variants” is inconsistent with the listed modes.** The list is `none`, `api_key`, `bearer`, `basic`, `oauth2_client_credentials`, `oauth_passthrough`: six modes including `none`. This should be named consistently to avoid test and docs drift.

- **MEDIUM: Secret expansion needs explicit tests.** The plans assume `${TFL_APP_KEY}` / `${PMCP_HMAC_SECRET}` are resolved by existing config/secrets machinery before outbound auth applies them. Add tests proving outbound auth receives resolved values, not literal `${...}` strings.

---

## 90-01 — HTTP Connector + Outgoing Auth

### Summary
Strong foundational plan. It correctly separates outbound HTTP auth from inbound MCP auth, establishes redaction discipline, and captures the important URL path-concat pitfall.

### Strengths
- Clear `HttpConnector` trait analogous to `SqlConnector`.
- Explicit redaction tests for URL/token/header leakage.
- Correctly owns feature grammar early.
- Good regression coverage for API Gateway stage-prefix preservation.

### Concerns
- **HIGH:** `oauth_passthrough` cannot be implemented correctly with a startup-created `Arc<dyn HttpAuthProvider>` unless the provider can access per-request inbound auth context.
- **HIGH:** `reqwest` feature `query` may not exist; verify against the actual workspace reqwest version before encoding it into the feature list.
- **MEDIUM:** OAuth2 client-credentials behavior needs token fetch/cache/refresh/redaction semantics, not just config enum lifting.
- **MEDIUM:** If `base64` is optional, the `http` feature must include `dep:base64`.

### Suggestions
- Add a small auth architecture note: static auth providers vs per-request passthrough providers.
- Add tests for `required=false` behavior for missing bearer/api key.
- Validate dependency feature names with `cargo metadata` or a minimal `cargo check`.

### Risk Assessment
**MEDIUM-HIGH.** Connector/auth is the right scope, but passthrough auth and dependency feature correctness could block downstream plans.

---

## 90-02 — Config Additions

### Summary
Good additive config plan, but it should not be in the same dependency wave as Plan 01 unless Plan 01 is guaranteed to finish first.

### Strengths
- Preserves `deny_unknown_fields`.
- Keeps SQL config compatibility as an explicit test.
- Adds the necessary `ToolDecl` fields for D-01.
- Reuses existing `ParamDecl`, avoiding unnecessary schema churn.

### Concerns
- **HIGH:** Hidden dependency on Plan 01’s `http` module types.
- **MEDIUM:** Ambiguous tool declarations are not addressed: `sql` + `script`, or `script` + `path/method`. “Script wins” may be okay, but should be validated or rejected explicitly.
- **LOW:** `method` as free-form `String` allows typos until execution.

### Suggestions
- Set `depends_on: [90-01]`, or define config-local stub types first.
- Add validation for mutually exclusive tool kinds.
- Normalize/validate HTTP methods during config validation.

### Risk Assessment
**MEDIUM.** The design is sound, but dependency ordering and ambiguous config handling need tightening.

---

## 90-03 — OpenAPI Parser + Single-Call Tools

### Summary
This plan completes the simple curated-tool path and correctly treats specs as optional runtime inputs. It needs more precision around operation/base URL modeling.

### Strengths
- Good parser coverage for JSON and YAML.
- Explicitly keeps curated-only boot independent of specs.
- Reuses existing `ToolInfo`/schema synthesis helpers.
- Avoids `ToolInfo` struct literals.

### Concerns
- **MEDIUM:** `Operation` starts in Plan 01 and is “unified” here. That can cause avoidable churn; the authoritative type should probably live in one place from the start.
- **MEDIUM:** Per-tool `base_url` is listed in config but not clearly reflected in `HttpConnector::execute`.
- **MEDIUM:** OpenAPI `$ref`, parameter location conflicts, request-body schema mapping, and header params are not strongly tested.
- **LOW:** Temporary script-tool error path may become obsolete immediately in Plan 05.

### Suggestions
- Add tests for per-tool `base_url`, header params, query params, path params, and request bodies.
- Keep `Operation` in `http::schema` and re-export it from day one.
- Add a negative test for missing `path` or `method`.

### Risk Assessment
**MEDIUM.** The simple path is well scoped; correctness depends on operation modeling details.

---

## 90-04 — HttpCodeExecutor + OAPI-10 Refactor

### Summary
This is the riskiest technical plan and the most important one. The intent is exactly right, but the feature gate and file ownership need correction.

### Strengths
- Correctly identifies the hardcoded SQL executor/flavor as the key refactor.
- Protects SQL with downstream `pmcp-sql-server` tests.
- Makes validation flavor part of the refactor, not an afterthought.
- Preserves the one wiring function goal.

### Concerns
- **HIGH:** Feature gate contradicts the planned umbrella. `HttpCodeExecutor` should be gated on `openapi-code-mode` if it references `pmcp_code_mode::HttpExecutor` exports behind `js-runtime`.
- **HIGH:** The files modified list omits SQL caller files that must change if signatures change.
- **HIGH:** OpenAPI validation flavor is under-specified. The plan says use `JavaScriptValidator` but does not pin the exact API or assert real JS validation behavior.
- **HIGH:** `oauth_passthrough` still lacks request context in `HttpCodeExecutor`.

### Suggestions
- Gate `HttpCodeExecutor`, OpenAPI flavor tests, and related exports under `#[cfg(feature = "openapi-code-mode")]`.
- Add `pmcp-sql-server/src/assemble.rs` or any actual callers to `files_modified`.
- Test OpenAPI `validate_code` with valid JS, invalid JS, and disallowed operation/policy cases.
- Decide passthrough auth before implementing this seam.

### Risk Assessment
**HIGH.** This is the phase’s critical integration point. Fix the gate and passthrough design before execution.

---

## 90-05 — Script Tools + Engine Parity

### Summary
The plan directly targets D-01/D-02 and includes the right parity proof. It should be tightened around argument validation and API stability from Plan 03.

### Strengths
- Correctly uses the same `PlanCompiler`/`PlanExecutor` path as Code Mode.
- Explicitly avoids a validate/token cycle for admin-authored scripts.
- Includes a first-class parity test: same script, same executor, same output.
- Correctly gates on `openapi-code-mode`.

### Concerns
- **HIGH:** “Args schema-validated first” may be assumed rather than implemented. If `ToolHandler::handle` can be called directly, the handler should validate or tests should prove the server layer validates before calling it.
- **MEDIUM:** Plan 05 changes the synthesizer signature created in Plan 03. That API churn should be anticipated in Plan 03.
- **MEDIUM:** Compiling the script on every call is simple but potentially expensive; acceptable initially, but note it.
- **MEDIUM:** CodeExecutor variable binding must be verified carefully so `args` is identical in both paths.

### Suggestions
- Add explicit runtime validation or a test through the real MCP tool-call path.
- Consider compiling the plan once in `ScriptToolHandler::new` if the plan type is safe to store.
- Include the light-build compile test in the automated verification, not only acceptance text.

### Risk Assessment
**MEDIUM-HIGH.** The design satisfies D-02, but validation and signature integration need care.

---

## 90-06 — `pmcp-openapi-server` Binary

### Summary
Good Shape A plan. It mirrors SQL appropriately and keeps `--spec` optional. Main risks are test/process ergonomics and behavior when Code Mode is enabled without a spec.

### Strengths
- Correctly avoids lifting reference `pmcp_server.rs`.
- Uses streamable HTTP only.
- Preserves lazy startup.
- Wires single-call tools, script tools, and Code Mode through the toolkit.

### Concerns
- **HIGH:** `cargo run -p pmcp-openapi-server --example openapi_server_min` may hang if the example starts a server. Use `cargo check --example` or a bounded smoke test.
- **MEDIUM:** Behavior for `[code_mode] enabled=true` with no `--spec` is not defined. Does Code Mode run without `api_schema`, warn, or fail?
- **MEDIUM:** Test server lifecycle/shutdown is not described; spawned servers can leak across tests.
- **LOW:** `DispatchError` redaction should include backend base URL tests.

### Suggestions
- Define no-spec + code-mode behavior explicitly.
- Make examples build-only in CI unless they self-exit.
- Add shutdown guards for `run_serving` tests.

### Risk Assessment
**MEDIUM.** The architecture is straightforward once Plans 03-05 are stable.

---

## 90-07 — London Tube Parity

### Summary
Good canonical parity target. It exercises the new query-param API key path and keeps default CI offline.

### Strengths
- Correctly chooses London Tube over Lichess for auth coverage.
- Uses wiremock, not live network or Docker.
- Drives the real binary path.
- Includes env-gated live replay.

### Concerns
- **MEDIUM:** If `${TFL_APP_KEY}` expansion is not proven, the query-param auth assertion may fail or assert the wrong value.
- **MEDIUM:** Manually vendored scenarios can drift from the reference unless their provenance is captured.
- **LOW:** `required=false` plus dummy key needs a clear expected behavior: omitted when unset, present when set.

### Suggestions
- Add an assertion that `app_key=dummy` appears, not `${TFL_APP_KEY}`.
- Include reference commit/path metadata in the fixture comments.
- Add a no-live-network guard grep or test convention.

### Risk Assessment
**MEDIUM.** Mostly integration-test complexity, not design risk.

---

## 90-08 — `cargo pmcp new --kind openapi-server`

### Summary
The scaffold plan matches the milestone goal, but it risks duplicating binary assembly logic and drifting from `pmcp-openapi-server`.

### Strengths
- Correctly emits a single runnable crate.
- Uses `openapi-code-mode`, so script tools and Code Mode compile.
- Preserves dev-only token-secret posture.
- Keeps deploy target enum unchanged.

### Concerns
- **MEDIUM:** Generated `main.rs` may duplicate assembly logic instead of reusing a stable helper, increasing drift risk.
- **MEDIUM:** `files_modified` omits README/help docs even though the plan updates them.
- **MEDIUM:** Deploy asset bundling for `config.toml` and `api.yaml` is not explicitly tested.
- **LOW:** Golden “≤15-line” tests can become brittle if formatting changes.

### Suggestions
- Prefer calling a stable library helper from `pmcp-openapi-server` or toolkit where possible.
- Add README/help files to the plan metadata.
- Test that scaffolded deploy includes both config and spec assets.

### Risk Assessment
**MEDIUM.** Achievable, but template drift and deploy asset correctness need guardrails.

---

## 90-09 — Docs

### Summary
Appropriate final docs plan with the right three surfaces. Low implementation risk.

### Strengths
- Leads with `cargo pmcp`, matching the intended user path.
- Covers binary, scaffold, tool kinds, auth, and spec optionality.
- Uses mdbook builds as verification.
- Keeps code fences doctest-safe.

### Concerns
- **LOW:** Docs can drift from actual CLI if generated help is not checked.
- **LOW:** `mdbook` availability may be an environment assumption.

### Suggestions
- Include a small generated-help comparison or grep against `cli.rs`.
- Add link checks if the repo already has a docs link-check command.

### Risk Assessment
**LOW.** Mostly editorial once prior plans land.

---

## Final Risk Assessment

**Overall risk: MEDIUM-HIGH.**

The plans are complete enough to deliver the phase goal, including the key D-01/D-02 requirement that curated script tools and Code Mode share the same engine. The main blockers to resolve before execution are:

1. Fix Plan 04’s `openapi-code-mode` feature gate.
2. Add an explicit per-request design for `oauth_passthrough`.
3. Make Plan 02 depend on Plan 01 or move shared config types.
4. Strengthen OAPI-10 tests so SQL remains green and OpenAPI JS validation actually runs.
5. Define no-spec + Code Mode behavior.

With those corrected, the wave structure and scope look defensible.

---

## Consensus Summary

Two reviewers, sharply different overall verdicts: **Gemini = LOW risk**, **Codex = MEDIUM-HIGH**.
Gemini reviewed at the architecture/feature-gating level and found the design clean. Codex went
deeper into per-request data flow and cross-plan dependency ordering and found real gaps. Where
they agree, confidence is high; where Codex stands alone, those are the items most worth resolving
before execution because they are exactly the blind spots a single reviewer (or the plan-checker)
missed.

### Agreed Strengths (both reviewers)
- **D-02 engine unification is correct** — reusing the SDK's pure-Rust `PlanCompiler`/`PlanExecutor`
  for both script tools and Code Mode (no second JS engine), with a first-class engine-parity proof
  in Plan 05.
- **Credential-redaction discipline** is consistent across all new error types (`HttpConnectorError`,
  `DispatchError`, `ExecutionError`).
- **OAPI-10 generalization intent is right** — widen `code_mode_tools_from_executor` to
  `Arc<dyn CodeExecutor>` + a validation flavor, protected by downstream SQL tests.
- **london-tube** is the right parity target (exercises api_key query-param auth), wiremock-only,
  live replay env-gated.
- **Feature-gating intent** (the `openapi-code-mode` umbrella keeps single-call builds SWC-free).

### Agreed Concerns (raised by both — address)
- **Plan 03 transient script-tool error state** (both LOW): the synthesizer returns a typed
  `ToolkitError` for script tools until Plan 05 lands; safe within a sequential build but leaves a
  transient gap if the pipeline halts mid-phase. Plan 05 changes Plan 03's synthesizer signature —
  anticipate that API churn in Plan 03.
- **Validation flavor under-specified / stringly-typed** (Gemini: prefer `enum ValidationFlavor { Sql, OpenApi }`
  over `&str`; Codex: OpenAPI `validate_code` behavior not pinned to a real JS-validation assertion).
- **Per-tool `ExecutionConfig` override** — both note it as a reasonable future additive enhancement;
  default-to-shared `[code_mode]` config is fine for now.

### Divergent Views — Codex-only HIGH findings (highest priority to resolve)
These are the items a second reviewer caught that the first (and the plan-checker) did not:

1. **HIGH — `oauth_passthrough` is architecturally under-specified.** Auth providers are created at
   dispatch/startup, but passthrough needs the *incoming MCP auth token per request*. Neither
   `HttpConnector::execute(operation, args)` nor `HttpExecutor::execute_request(method, path, body)`
   receives `RequestHandlerExtra`/`AuthContext`. As planned, `oauth_passthrough` would be a configured
   variant that cannot work. (Note: CONTEXT D-05 ships all five variants; the passthrough one needs a
   per-request token path designed in, or it should be explicitly deferred.)
2. **HIGH — Plan 04 feature gate conflicts with the umbrella.** The Wave-1 revision fixed the gate in
   Plans 01/05/06, but Codex flags that Plan 04 still gates `HttpCodeExecutor` under
   `all(feature="http", feature="code-mode")` and tests with `--features "http code-mode"` — which omits
   `pmcp-code-mode/js-runtime`. Plan 04 should gate on `openapi-code-mode` like 05. (This is the same
   class of bug the plan-checker caught for 01/05/06; Plan 04 may be a residual instance.)
3. **HIGH — Plan 02 is not independent of Plan 01.** Both are Wave 1, but Plan 02 references
   `crate::http::auth::AuthConfig` and `crate::http::client::HttpConfig` (created by Plan 01). If the
   wave runs in parallel, Plan 02 cannot compile until Plan 01 lands. Set `depends_on: [90-01]` (move
   to Wave 2) or define config-local stub types.

Other Codex-only items worth a quick check: "five auth variants" vs six listed modes (incl. `none`) —
naming drift; verify the `reqwest` `query`/`base64` feature names against the actual workspace version;
add tests proving `${ENV}` secrets are *resolved* before outbound auth applies them; define
`[code_mode] enabled=true` + no `--spec` behavior (run without `api_schema`? warn? fail?); make the
runnable example build-only / self-exiting so smoke tests don't hang.

### Recommended Action
Several Codex HIGH items (#1 passthrough, #2 Plan 04 gate, #3 Plan 02 dependency) are concrete and
fixable. Run `/gsd:plan-phase 90 --reviews` to replan incorporating this feedback — the planner will
read this REVIEWS.md and tighten the affected plans before execution.
