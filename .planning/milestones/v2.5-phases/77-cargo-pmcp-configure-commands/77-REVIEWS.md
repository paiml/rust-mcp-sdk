---
phase: 77
reviewers: [gemini, codex]
reviewed_at: 2026-04-26T16:59:09Z
plans_reviewed: [77-01-PLAN.md, 77-02-PLAN.md, 77-03-PLAN.md, 77-04-PLAN.md, 77-05-PLAN.md, 77-06-PLAN.md, 77-07-PLAN.md, 77-08-PLAN.md, 77-09-PLAN.md]
---

# Cross-AI Plan Review — Phase 77 (cargo pmcp configure commands)

Reviewed by Gemini and Codex (Claude excluded per workflow — current orchestrator is Claude Code).

---

## Gemini Review

# Phase 77 Plan Review: cargo pmcp configure commands

This review evaluates the 9-plan sequence for Phase 77, which implements the `cargo pmcp configure` command group.

## 1. Summary
The implementation plans for Phase 77 are of exceptional quality. They demonstrate a deep understanding of the existing codebase, rigorously follow established patterns (particularly the Phase 74 auth cache), and prioritize security through a "references-only" secrets policy. The strategy for resolving the `--target` flag collision by renaming the legacy selector to `--target-type` with a deprecation alias is a textbook example of safe CLI evolution. The testing architecture is comprehensive, spanning unit, property, and fuzz tests, along with a worked example that validates the primary monorepo use case.

## 2. Strengths
*   **Safe Flag Evolution:** Renaming the existing `--target` (type selector) to `--target-type` while maintaining a one-cycle alias (`alias = "target"`) avoids breaking existing CI/CD pipelines while freeing the flag for its more intuitive "named target" semantic.
*   **Pattern Reuse:** Wholesale cloning of the Phase 74 `auth_cmd/cache.rs` logic for atomic writes, Unix permissions (`0o600`), and schema versioning ensures that the configuration system is as robust and secure as the auth system.
*   **Zero-Touch Backward Compatibility:** The logic in Plan 06/07 ensures that users without a `~/.pmcp/config.toml` experience exactly the same behavior as today, fulfilling the "don't break zero-config" mandate.
*   **Security Validation:** The proactive raw-credential validator in `configure add` (Plan 04) effectively mitigates the risk of secrets leaking into the user-level configuration file.
*   **Excellent Documentation:** The commitment to rustdoc with examples and the clear mapping of requirement IDs (REQ-77-XX) ensures long-term maintainability and traceability.

## 3. Concerns
*   **Precedence Order (Flag vs. Env):**
    *   **Severity: MEDIUM**
    *   **Detail:** Plan 06 (D-04) specifies a precedence of `ENV > flag > target > deploy.toml`. This is the **reverse** of standard CLI conventions (including the `aws-cli` north star), where an explicit command-line flag (`--target dev`) almost always overrides an environment variable (`PMCP_TARGET=prod`).
    *   **Risk:** Users may be confused if they explicitly pass a target name via a flag, only to have it ignored because of a stale environment variable in their shell. While the plan justifies this with a CI use case (ENV overriding a "forgotten" flag), this behavior is unconventional and prone to user error.
*   **Filename Collision Confusion:**
    *   **Severity: LOW**
    *   **Detail:** Both the new user-level file (`~/.pmcp/config.toml`) and the existing workspace-level file (`.pmcp/config.toml` for secrets) share the same name.
    *   **Risk:** While paths differ, a developer running `cat .pmcp/config.toml` might be confused when they don't see their deployment targets. The plan mitigates this via prefixes in documentation, but it remains a minor UX friction point.
*   **Missing `configure use --none` or `unuse`:**
    *   **Severity: LOW**
    *   **Detail:** There is no planned way to clear the workspace marker via the CLI (returning the workspace to the "default" or "no target" state).
    *   **Risk:** Users must manually delete `.pmcp/active-target` to revert a workspace's selection.

## 4. Suggestions
*   **Swap Precedence:** Highly recommend changing the precedence to **`flag > ENV > target > deploy.toml`**. This aligns with the "principle of least surprise" in CLI design, where the most specific/explicit instruction (the flag) wins.
*   **Interactive Confirmation on `use`:** When running `configure use <name>`, if a marker already exists for a *different* target, consider printing a brief confirmation/note (unless `--yes` or `--quiet` is set) to ensure the user is aware they are switching environments.
*   **Credential Validator Escape Hatch:** Ensure the `--allow-credential-pattern` flag is prominently mentioned in the error message when a value is rejected, as users may legitimately need to store strings that look like patterns (e.g., in a test environment).

## 5. Risk Assessment: LOW
The risk is low due to the modular design, the reuse of proven atomic-write patterns, and the heavy emphasis on automated validation (fuzzing and property tests). The most significant risk is the counter-intuitive precedence order, which is a UX risk rather than a technical or security risk. The mechanical rename of the legacy `--target` flag is well-mapped and carries minimal regression risk.

**Verdict: Approved for implementation, with a recommendation to revisit the Flag-vs-Env precedence order.**

---

## Codex Review

## Summary

The plan set is strong on research depth, traceability, and test intent, but I would not call it execution-safe yet. The biggest issue is an architectural mismatch between the current `cargo-pmcp` lib/bin split and the plan’s proposed testing/example strategy, plus a real scope hole around non-`deploy` target consumers. The result is a plan that is directionally correct, but still carries a few high-probability integration failures that could force mid-phase replanning.

## Strengths

- The phase is well-scoped at the product level: named targets, workspace marker, precedence rules, and backward compatibility are all explicit.
- The plans are unusually strong on traceability. REQ mapping, plan dependencies, and validation coverage are clear.
- The research identified the real `--target` collision early and proposed the correct general direction: split named target selection from target-type selection.
- Security posture is thoughtful: references-only config, raw-credential rejection, atomic writes, and Unix perms are the right defaults.
- The plan reuses existing repo patterns instead of inventing new ones, especially around atomic file writes and clap command-group structure.
- The operator-facing banner and `configure show` attribution model are good product choices if implemented precisely.

## Concerns

- **HIGH**: The plan assumes `cargo_pmcp::commands::configure::*` will be usable from integration tests and examples, but the current lib target does not expose `commands` at all. [cargo-pmcp/src/lib.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/lib.rs:1) explicitly keeps the full command tree out of the lib surface, and [cargo-pmcp/src/main.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/main.rs:21) mounts `commands` as a private bin module. Plans 03 and 08 depend on a structural change that is not actually planned.
- **HIGH**: The integration scope is incomplete relative to the phase goal. The roadmap/context says this must integrate with both `deploy` and `pmcp.run upload` flows, but Plan 07 only wires `deploy/mod.rs`. The research already identified `test/upload.rs`, `loadtest/upload.rs`, and `landing/deploy.rs` as banner/resolver call sites, and those are not covered.
- **HIGH**: Resolver design is internally inconsistent and under-models the data. Plan 06 tests mention deploy.toml fall-through for `account_id`, but `ResolvedTarget` only carries `api_url`, `aws_profile`, and `region`. That means target-specific fields are not actually resolved or attributed, which weakens both `configure show` and future non-`pmcp-run` targets.
- **MEDIUM**: `configure show <name>` is not correctly modeled. The planned `show` enrichment resolves the active target, then only uses that result if the resolved name matches the requested name. So `show prod` while `dev` is active will not produce true merged attribution for `prod`; it falls back to raw-ish target output.
- **MEDIUM**: The plan resolves and injects target env vars in `main.rs` before every subcommand, including non-target-consuming commands. That creates avoidable failure modes and side effects for commands like `auth`, `doctor`, or even `configure` itself when a target is missing or malformed.
- **MEDIUM**: The `--target`/`--target-type` clap story is still risky. Keeping `--target` as a deploy-scoped alias while also introducing a top-level named-target `--target` is exactly the kind of ambiguity clap can make painful. The plan acknowledges this, but treats it as something to discover in implementation rather than lock down first.
- **MEDIUM**: The banner/note contract drifts from the phase decisions. D-03 requires the override note only when `PMCP_TARGET` overrides a workspace marker, and D-13 gives exact source-line wording. The planned implementation emits a generic env note whenever the source is env, and the source text does not match the specified format.
- **LOW**: There is plan drift across files. Some steps still call old resolver signatures like `resolve_target(None)`, helper signatures differ between sections, and `files_modified` inventories do not always match the described edits. That is not fatal, but it is a sign the plan set needs one consistency pass before execution.

## Suggestions

- Add a short prerequisite plan to resolve the lib/bin boundary explicitly. Either:
  - keep `configure` bin-only and write integration tests as subprocess E2E tests via `CARGO_BIN_EXE_cargo-pmcp`, or
  - intentionally expose a shared CLI/config module from the lib and pay the refactor cost up front.
- Expand the integration wave to cover all target-consuming flows already identified in research: `deploy`, `test/upload`, `loadtest/upload`, and `landing/deploy`. If that is too much for Phase 77, narrow the phase goal and requirements now.
- Redesign `ResolvedTarget` before coding. It should either:
  - carry all common and variant-specific resolved fields plus source attribution, or
  - expose a generic field-attribution map so `show` can be truthful for every target type.
- Split “target selection resolution” from “field merge resolution”. `configure show <name>` needs a resolver that can operate on an explicit named target, not only the currently active one.
- Do not resolve/inject targets globally for every command in `main.rs`. Gate that behavior to target-consuming commands only.
- Replace the `--target` alias grace period with a proven clap parse matrix before implementation. Add snapshot tests for:
  - `cargo pmcp --target dev deploy --target-type aws-lambda ...`
  - `cargo pmcp deploy --target aws-lambda ...`
  - failure/help cases.
- Add exact-output snapshot tests for the D-03 override note and D-13 banner/source strings. Those formats are product behavior, not implementation detail.

## Risk Assessment

**HIGH**

The planning work is thoughtful, but there are still two material execution blockers: the lib/bin architecture mismatch and the missing upload-flow integration. On top of that, the resolver contract is not fully aligned with the phase’s own field-attribution promises. Those are not cosmetic issues; they are the kind that cause mid-implementation design churn. If those are corrected before coding starts, the phase risk drops substantially.

---

## Consensus Summary

### Agreed Strengths

- **Pattern reuse from Phase 74** — both reviewers commend the wholesale clone of `auth_cmd/cache.rs` (atomic writes, schema versioning, Unix 0o600 perms).
- **Security posture** — references-only D-07 policy, raw-credential validator, atomic writes are correct defaults.
- **Traceability** — REQ-77-XX mapping, dependency graph, and validation coverage are well-structured.
- **Safe `--target` flag evolution** — rename-to-`--target-type` with one-cycle alias is the right move (acknowledged by both, though Codex flags clap-parsing risk that needs a snapshot-test matrix).

### Agreed Concerns

- **`configure show <name>` does not correctly resolve a non-active named target** (Codex MEDIUM). The plan resolves the active target, then short-circuits if the requested name doesn't match — `show prod` while `dev` is active falls back to raw output without true source attribution. Both reviewers want this fixed before coding.
- **D-03 override-note and D-13 banner format are exact product behavior** and need snapshot tests, not just "banner emitted" greps.
- **Credential-validator escape hatch** must be discoverable — the rejection error message should reference `--allow-credential-pattern` (Gemini suggestion; benign UX fix).

### Codex-only HIGH-severity concerns (Codex risk = HIGH; Gemini risk = LOW)

These are issues Gemini missed but Codex flagged as execution blockers:

1. **HIGH — lib/bin boundary**: `cargo-pmcp/src/lib.rs` does NOT export `commands::*`. `commands` is mounted as a private bin module in `main.rs`. Plan 03's `test_support_configure` re-export pattern AND Plan 08's example/integration test imports both depend on a lib-surface exposure that is NOT planned. Fix: either (a) keep `configure` bin-only and write integration tests as subprocess E2E via `CARGO_BIN_EXE_cargo-pmcp`, or (b) intentionally expose a shared CLI/config module from the lib and pay the refactor cost in Plan 03.
2. **HIGH — integration scope incomplete**: Roadmap/CONTEXT says Phase 77 must integrate with `deploy` AND `pmcp.run upload`. RESEARCH §7 enumerates banner/resolver call sites in `test/upload.rs`, `loadtest/upload.rs`, `landing/deploy.rs` — Plan 07 only wires `deploy/mod.rs`. Either expand Wave 5 or narrow the phase requirements explicitly.
3. **HIGH — `ResolvedTarget` under-models target-specific fields**: It only carries `api_url`, `aws_profile`, `region`. Plan 06 tests mention deploy.toml fall-through for `account_id`, but `account_id` isn't on the struct. Non-`pmcp-run` targets (aws-lambda, google-cloud-run, cloudflare-workers) have variant-specific fields that have no resolution or attribution path. Redesign: either carry all common+variant fields, OR expose a generic field-attribution map so `show` can be truthful for every target type.

Additional Codex MEDIUM concerns worth landing before execution:
- Target env injection in `main.rs` runs for every subcommand including `auth`, `doctor`, `configure` itself — should be gated to target-consuming commands.
- clap matrix for `--target` (top-level) vs `--target-type` (deploy-scoped, with `alias = "target"`) needs snapshot tests for ambiguity edge cases (e.g. `cargo pmcp --target dev deploy --target-type aws-lambda`).
- Plan drift LOW: stale resolver signatures and `files_modified` inventory inconsistencies — one consistency pass needed.

### Divergent Views

- **Precedence order (D-04)**: Gemini suggests swapping to `flag > ENV > target > deploy.toml` per "principle of least surprise". This conflicts with **CONTEXT.md D-04 LOCKED** decision (`ENV > flag > target > deploy.toml` matches `aws-cli`'s precedence; the user explicitly chose ENV-wins because CI workflows with `AWS_REGION` env should override an accidental `--region` in a script). The lock stands; Gemini's suggestion is overridden by the user's documented rationale. **No action required.**
- **Risk verdict**: Gemini = LOW ("approved for implementation"); Codex = HIGH ("not execution-safe yet, mid-implementation churn likely"). Codex's HIGH is the conservative read; the lib/bin and integration-scope concerns are concrete and verifiable, so the divergence is best resolved by addressing Codex's HIGH items rather than picking a verdict.

### Recommended Action

Run `/gsd-plan-phase 77 --reviews` to feed REVIEWS.md back to the planner for a third revision cycle covering Codex's 3 HIGH concerns + 4 MEDIUM concerns. Gemini's LOW-risk suggestions can be folded in opportunistically (escape-hatch error message, interactive confirmation on `use`).

