---
phase: 85
reviewers: [gemini, codex]
reviewed_at: 2026-05-26
plans_reviewed: [85-01-PLAN.md, 85-02-PLAN.md, 85-03-PLAN.md, 85-04-PLAN.md, 85-05-PLAN.md, 85-06-PLAN.md]
---

# Cross-AI Plan Review — Phase 85

## Gemini Review

# Phase 85 Plan Review: Shape A Pure-Config Binary + Reference Parity

## 1. Summary
The implementation plans for Phase 85 are exceptionally well-structured, providing a clear 4-wave progression from toolkit foundation fixes to the final result-parity proof. The strategy of resolving the identified research gaps (REF-01 superset fields and code-mode registration) in the toolkit crate before building the binary ensures that `pmcp-sql-server` remains a thin, maintainable glue layer. By vendoring the Chinook DDL and scenarios, the plans guarantee a self-contained CI process that proves the "zero-Rust" value proposition against a production-grade reference without requiring external cloud credentials.

## 2. Strengths
*   **Gap-First Foundation:** Identifying and planning the toolkit-level gaps (REF-01 fields, `${VAR}` expansion, and `SqlCodeExecutor`) in Waves 1 and 2 ensures that the binary itself doesn't become a dumping ground for logic that belongs in the SDK.
*   **Lazy Connection Verification:** The explicit planning of the `SC-1` (lazy-startup) test is a critical guard against CI regressions where a backend driver might accidentally trigger a network round-trip during metadata synthesis.
*   **Defense-in-Depth Security:** The `SqlCodeExecutor` correctly implements re-validation of SQL statements before execution, mirroring the production security model and mitigating token-replay risks.
*   **Self-Contained Parity Harness:** Using `mcp-tester` as a library and vendoring the Chinook DDL allows for high-fidelity result parity testing (SC-4) in a hermetic environment.
*   **Strict Error UX:** The dispatch logic explicitly handles compiled-out features with actionable guidance, which is essential for a "Universal Binary" distributed via `cargo install`.

## 3. Concerns
*   **Crate Version Ripple (MEDIUM):** 85-03 Task 1 requires verifying version strings for unpublished Phase 84 crates. If these weren't bumped consistently during Phase 84, the `Cargo.toml` wiring could fail.
    *   *Mitigation:* The task includes a "read_first" gate for these manifests; the executor must ensure they match the local workspace state.
*   **HMAC Secret Length Enforcement (LOW):** While `HmacTokenGenerator` enforces a 16-byte minimum, the `${VAR}` expansion in 85-01/02 could surface this as a startup error if the environment isn't seeded correctly.
    *   *Mitigation:* 85-06 Task 1 explicitly sets a compliant test secret in the harness.
*   **Shape C Scope Overlap (LOW):** 85-06 Task 2 includes a "Shape C ≤15-line example" to satisfy the ALWAYS matrix, while Phase 86 is dedicated to Shape C.
    *   *Mitigation:* This is acceptable as a crate-level example, but care should be taken not to "pre-ship" the final Phase 86 developer experience until that phase's specific requirements are refined.

## 4. Suggestions
*   **Schema Resource Header (D-05):** In 85-05 Task 1, consider prepending a small Markdown header to the `--schema` resource (e.g., `"# Database Schema\n\n"`). This helps LLMs identify the content type more reliably than a raw DDL dump.
*   **Fuzz Corpus Diversity:** When adding the `seed-chinook-superset.toml` in 85-06, ensure it includes various combinations of `${VAR}` and `env:VAR` to exercise the expanded resolution logic thoroughly.
*   **Dispatch Helper Granularity:** Given the cognitive complexity limit (≤25), ensure `dispatch_athena` specifically handles the construction of the `AthenaConfig` builder chain, as it is significantly more complex than the SQLite/Postgres arms.

## 5. Risk Assessment
**Overall Risk: LOW**

The phase relies almost entirely on previously validated toolkit primitives and connectors. The dependency ordering is sound, and the success criteria are objective and measurable via the mcp-tester replay. The transition from the `pmcp-run` internal patterns to the public toolkit is handled surgically, and the security model (policy enforcement + token binding) is maintained at parity with the production implementation. The most complex logic (`translate_placeholders`) was already landed in Phase 84, leaving Phase 85 as a high-confidence integration phase.

---

## Codex Review

## Summary

The phase plan is strong in sequencing and test intent, but I would not approve it as execution-ready yet. The foundation → binary → parity waves are sensible, and the plans correctly identify the major gaps in config parsing, code-mode registration, backend dispatch, and HTTP serving. The main risk is that the parity proof can still fail or prove the wrong thing: the Chinook fixture plan uses schema-only DDL without data, the server assembly risks dropping existing configured resources/prompts, and the code-mode executor/registration API is not resolved cleanly enough for downstream plans to depend on it.

## Strengths

- Clear 4-wave dependency structure. Plan 85-01 and 85-03 correctly unblock parser/fixture work before binary assembly.
- Good REF-01 discipline: additive fields, preserving `deny_unknown_fields`, and negative tests for renamed fields.
- Good security awareness around token-secret handling, inline-secret rejection, policy enforcement, and dispatch error redaction.
- The plan correctly avoids Docker/cloud dependencies and keeps non-SQLite coverage to parse/lazy startup.
- Using `mcp-tester` library replay is the right parity mechanism, and `ScenarioResult.success` matches the current API.
- Streamable HTTP reuse is correct; the plan avoids reimplementing transport.

## Concerns

- **HIGH: DDL-only Chinook fixture will not satisfy parity.**
  The generated scenarios assert real data values like `"Rock"`, `"Angus Young"`, `"AC/DC"`, and `"Bohemian"`. Loading only `.schema` creates empty tables, so curated tool calls will fail. Plan 03/06 needs a data-bearing fixture: either commit `chinook.db` with package exclusion, or vendor a full `.dump` including inserts.

- **HIGH: Server assembly may drop required configured resources.**
  Plan 05 builds a `StaticResourceHandler` from only the new schema resource. The reference config has at least `docs://chinook/schema`, `docs://chinook/examples`, and `code-mode://learnings`; generated.yaml reads all three. Assembly must start from `cfg.resources` and replace/override only the schema resource content with `--schema`.

- **HIGH: Prompt/resource wiring risks diverging from the reference config.**
  The reference `start_code_mode` prompt includes multiple resources. Plan 05 describes registering a generated prompt body directly, but the existing toolkit prompt model resolves configured `include_resources`. If Shape A replaces that with a standalone prompt, parity may drift. Preserve configured prompts and resolve them against the merged resource handler.

- **HIGH: Code-mode registration API is still ambiguous.**
  `register_code_mode_tools(builder, &config)` currently has no connector/executor input, but `execute_code` needs an executor. Plan 02 lists alternatives instead of locking one. This should be decided before execution. Recommended shape: add an executor-aware API such as `try_code_mode_from_config_with_connector` or `try_code_mode_from_config_with_executor`, and have Shape A use that explicitly.

- **HIGH: Parity test bypasses too much of Shape A.**
  Plan 06 injects an in-memory connector directly instead of exercising `--config --schema` through dispatch. That proves `build_server`, not the pure-config binary path. Add an end-to-end test that writes a temp config with `file_path` pointing to the seeded DB, invokes the real run/serve path, and replays scenarios through HTTP.

- **MEDIUM: `${VAR}` expansion scope is too narrow.**
  Plan 01 implements `${VAR}` only for `token_secret`, but reference configs also use shell-style variables elsewhere, such as Athena output locations. If the claim is "configs unchanged," expansion should be a general config-loading concern or the plan should explicitly justify token-secret-only expansion.

- **MEDIUM: Athena "lazy startup" is not fully guaranteed.**
  Current Athena construction calls AWS config loading before first query. That may still touch provider-chain behavior in CI. SC-1 should either harden Athena connector construction to be truly offline or use a mocked/dev constructor for lazy tools/list tests.

- **MEDIUM: Code-mode result shape may not match production.**
  Plan 02 proposes returning `{"rows": values}` from `SqlCodeExecutor`. If production/reference scenarios expect a different execute_code shape, parity can fail. The adapter should mirror the production observable output, not just the toolkit trait's convenient return shape.

- **LOW: Shape C example is mild scope creep.**
  Phase 86 owns Shape C. Keeping a runnable example for ALWAYS coverage is fine, but avoid making the ≤15-line Shape C contract a Phase 85 blocker beyond a small smoke example.

## Suggestions

- Replace `chinook.ddl` with either `chinook.db` or `chinook.dump.sql` containing schema plus data. Keep it out of the published crate via `exclude`.
- Add a `merge_schema_resource(cfg, schema_ddl)` helper that clones all configured resources and replaces only `docs://.../schema`.
- Build prompts from `cfg.prompts` using `StaticPromptHandler::from_configs(&cfg.prompts, &merged_resources)` so `start_code_mode` remains config-compatible.
- Lock the code-mode API before implementation. Prefer:
  `try_code_mode_from_config_with_connector(cfg, connector)` → builds `SqlCodeExecutor` → registers `validate_code` and `execute_code`.
- Keep `try_code_mode_from_config(cfg)` as the current validation/no-op path or clearly document it as connectorless validation only.
- Add a true CLI-path integration test: temp DB + temp config file + real `Args { config, schema, http: 127.0.0.1:0 }` + HTTP initialize/replay.
- Expand `${VAR}` at raw TOML load time, or add a documented `ServerConfig::from_toml_with_env_expansion` used by the binary.
- Make SC-1 include a timeout guard around non-SQLite startup tests to catch accidental credential/network waits.

## Risk Assessment

**Overall risk: HIGH until corrected.**

The plan is directionally right and well researched, but three issues can directly invalidate the headline success criterion: schema-only fixtures lack data, assembly may omit required resources/prompts, and code-mode registration lacks a settled connector-aware API. Once those are fixed, the risk drops to medium: remaining risks are mostly integration details around AWS lazy construction, env expansion scope, and exact code-mode output parity.

---

## Consensus Summary

Both reviewers agree the **4-wave foundation→binary→parity sequencing is sound** and the plans correctly identify the toolkit gaps. They diverge sharply on **overall risk: Gemini LOW vs Codex HIGH** — and that divergence is the most important signal in this review. Gemini reviewed the plans as written and found them internally consistent; Codex cross-checked the plans against the *actual content of the reference fixtures* (`generated.yaml`, the reference `config.toml`) and found five ways the parity proof could pass-but-prove-nothing or fail outright. Codex's HIGH findings are concrete and verifiable against files already in the repo, so they should be treated as the priority action list.

### Agreed Strengths
- **Foundation-first 4-wave structure** — both single out landing the REF-01/`${VAR}`/code-mode toolkit gaps (Waves 1–2) before the binary as the right call (Gemini "Gap-First Foundation"; Codex "Plan 85-01 and 85-03 correctly unblock…").
- **REF-01 additive discipline** — additive superset fields, `deny_unknown_fields` preserved, rename-rejection negative tests.
- **Security awareness** — token-secret handling, inline-secret rejection, static policy enforcement, dispatch-error redaction (both call this out explicitly).
- **mcp-tester library replay is the correct parity mechanism**, and no-Docker/no-cloud hermetic CI is correct.
- **Streamable-HTTP reuse** — neither wants transport re-implemented.

### Agreed Concerns
- **Shape C example overlaps Phase 86 (LOW)** — both flag it; both agree a small smoke example is acceptable, just don't pre-ship the Phase 86 ≤15-line contract as a Phase 85 blocker.
- **Athena construction nuance (LOW→MEDIUM)** — Gemini flags `dispatch_athena` complexity for the cog-25 budget; Codex flags that Athena's AWS-config loading may not be truly offline for the SC-1 lazy test. Same code path, two angles — harden or mock the Athena constructor for the lazy test.

### Divergent Views (investigate first — Codex-only HIGH findings)
These were caught by Codex's fixture cross-check and **not** surfaced by Gemini. Each is checkable against repo files before any replanning:
1. **DDL-only fixture has no data (HIGH).** `generated.yaml` asserts on real rows (`"Rock"`, `"Angus Young"`, `"AC/DC"`, `"Bohemian"`). `sqlite3 .schema` yields empty tables → curated tool calls return nothing → SC-3/SC-4 fail. Fix: vendor a **data-bearing** Chinook fixture (`chinook.db` or a full `.dump` with inserts), `exclude`d from the published crate. The `--schema` DDL file is still separate (it feeds the prompt/resource); the *connector* needs the populated DB.
2. **Assembly may drop configured resources (HIGH).** `generated.yaml` reads `docs://chinook/schema`, `docs://chinook/examples`, and `code-mode://learnings`. If Plan 05 builds the resource handler from only the new schema resource, `list_resources` + reads fail. Fix: start from `cfg.resources`, override only the schema resource with `--schema` content (a `merge_schema_resource` helper).
3. **Prompt wiring may diverge (HIGH).** The reference `start_code_mode` prompt resolves configured `include_resources`. Preserve configured prompts (`StaticPromptHandler::from_configs`) rather than registering a standalone generated body.
4. **Code-mode registration API unresolved (HIGH).** Plan 02 lists alternatives instead of locking one; `execute_code` needs an executor that `register_code_mode_tools(builder, &config)` can't currently receive. Lock a connector-aware API (`try_code_mode_from_config_with_connector`) before execution.
5. **Parity test bypasses the binary path (HIGH).** Plan 06 injects an in-memory connector → proves `build_server`, not `--config --schema` dispatch. Add a true end-to-end test: temp config with `file_path` → real run/serve → HTTP replay.
6. **`${VAR}` scope (MEDIUM)** and **execute_code result-shape parity (MEDIUM)** — confirm `${VAR}` covers Athena `output_location` (or justify token-secret-only), and confirm `SqlCodeExecutor`'s output shape mirrors production's observable `execute_code` payload, not just the trait's convenient return.

**Recommendation:** Items 1–5 are HIGH and most are verifiable in minutes against `pmcp-run/built-in/sql-api/reference/`. Run `/gsd:plan-phase 85 --reviews` to fold these into the plans before executing — especially the data-bearing fixture (1) and the resource/prompt-preservation fix (2/3), which together determine whether the parity replay proves anything at all.
