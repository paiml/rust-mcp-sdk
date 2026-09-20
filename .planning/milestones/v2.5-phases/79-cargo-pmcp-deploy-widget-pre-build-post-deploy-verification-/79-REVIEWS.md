---
phase: 79
reviewers: [codex, gemini]
codex_model: gpt-5 (codex-cli 0.128.0 default)
gemini_model: gemini-3-pro-preview
reviewed_at: 2026-05-03T17:35:00Z
plans_reviewed:
  - 79-00-PLAN.md
  - 79-01-PLAN.md
  - 79-02-PLAN.md
  - 79-03-PLAN.md
  - 79-04-PLAN.md
prompt_size_bytes: 310519
prompt_lines: 4458
prompt_includes: [PROJECT.md (first 80), ROADMAP Phase 79 entry, 79-CONTEXT.md, 79-RESEARCH.md, all 5 PLAN files]
---

# Cross-AI Plan Review — Phase 79

## Codex Review

**Summary**

The plans are strong on traceability, wave ordering, and scoping, and the build half looks close to shippable. The main weakness is Wave 3: the post-deploy verifier is relying on subprocess behavior, auth plumbing, and stdout parsing that do not match the current codebase. As written, Phase 79 probably closes A, partially closes B, and does not reliably close C without redesigning the verification seam.

**Strengths**

- Requirement coverage is disciplined. `79-00` gives a real traceability matrix instead of vague planning.
- Wave ordering is sensible. Schema first, build half second, verify half third, doctor/scaffold/docs last.
- The narrowed convention search to `widget/` and `widgets/` is a defensible default because it prefers false negatives over dangerous false positives.
- The explicit `embedded_in_crates` field is the right source of truth; demoting `include_str!` auto-detection to doctor-only is a good call.
- The plans take deploy-tool risk seriously: path traversal, auth leakage in `ps`, and "live but broken" operator UX are all surfaced instead of hand-waved.
- Pairing doctor warnings with scaffold changes is good product design: new users get the right template, existing users get migration guidance.
- Test planning is unusually thorough and generally aligned with the repo's quality culture.

**Concerns**

- `HIGH` The multi-widget cache-invalidation design is broken as planned. `79-02` sets one `PMCP_WIDGET_DIR`, and `79-04`'s generated `build.rs` watches one directory. With multiple `[[widgets]]` entries or one crate embedding more than one widget output dir, the last widget wins and Failure Mode B is still open.
- `HIGH` The verify-half parser path does not match the current CLI. The planned child argv includes `--quiet`, but the current test commands suppress human output when quiet is enabled in `cargo-pmcp/src/main.rs`, `cargo-pmcp/src/commands/test/check.rs`, `cargo-pmcp/src/commands/test/conformance.rs`, and `cargo-pmcp/src/commands/test/apps.rs`. Even without `--quiet`, the actual pretty output comes from `crates/mcp-tester/src/report.rs`, not the planned `8/8 tests passed` / `1/8 widgets failed` strings.
- `HIGH` The custom `resolve_auth_token` design fights the existing auth path in `cargo-pmcp/src/commands/auth.rs`. Today, `AuthMethod::None` already falls back to the Phase 74 cache and transparently refreshes near-expiry tokens. Converting that to a parent-side static bearer token loses refresh behavior and creates false infra failures in CI or under a different user account.
- `HIGH` `on_failure="fail"` is still too ambiguous for CI/CD. A loud banner helps humans, but machine consumers still need a distinct signal for "deploy succeeded and the bad revision is live" versus "deploy failed before cutover." Reusing generic nonzero behavior is not enough.
- `MEDIUM` The scaffold target is misaligned with the current `cargo pmcp app new` template in `cargo-pmcp/src/templates/mcp_app.rs`. That scaffold uses `WidgetDir` file-serving, not `include_str!`, so adding `build.rs` there is mostly harmless but does not address the actual stale-binary failure mode for new apps.
- `MEDIUM` The `node_modules/` heuristic is too blunt. Yarn PnP and some alternative install layouts legitimately omit `node_modules`, so "missing dir means install" will cause needless or incorrect installs in some real projects.
- `MEDIUM` The explicit `build` / `install` override design is underspecified. The plan parses shell-like commands by whitespace, which breaks quoting and flags, and it still requires a `package.json` build script even when an explicit build command is configured.
- `MEDIUM` The direct `cargo build` path remains unsafe by design. That may be acceptable for a deploy-scoped phase, but the plan sometimes talks as if Failure Mode B is fully solved; it is only solved on the deploy path.
- `LOW` The convention narrowing will miss member-local widget dirs and existing `ui/` layouts. That is acceptable only if the docs and doctor output point clearly to `[[widgets]]`.
- `LOW` The doctor check will be advisory at best. Literal `include_str!` scanning is fine for warnings, but it will miss macro/computed paths and may warn on examples/tests.

**Suggestions**

- Add a structured verification seam before implementing Wave 3. Either expose a library-returned `TestReport` or add a machine-readable mode to `cargo pmcp test {check,conformance,apps}`. Do not parse pretty terminal output.
- Redesign cache invalidation for multiple widgets. A single env var is not enough. Use a list contract such as `PMCP_WIDGET_DIRS`, or generate crate-specific watchers from `embedded_in_crates`.
- Let child `test` subprocesses resolve auth themselves using the existing cache/refresh logic. Only inject `MCP_API_KEY` when the user explicitly supplied it or when deploy truly owns a bearer token unavailable to the child.
- Give "live but broken" a dedicated machine signal. A unique exit code or JSON status block is better than relying on prose banners in CI logs.
- If explicit `build` / `install` remain string fields, either execute them through a shell intentionally or change the schema to argv arrays. Also skip the `package.json` build-script check when `build` is explicitly provided.
- Re-scope the scaffold work. Put `build.rs` only in scaffolds that actually embed widget bundles, or document clearly that `WidgetDir` projects usually do not need it.
- If schedule pressure is real, split 79-03. A/B can land cleanly first; C is the risky part.

**Risk Assessment**

Overall risk is `HIGH` as written. The build half should achieve A and likely B for the single-widget case, but the current plan does not reliably solve B for multi-widget projects and the verify half has real design mismatches with the existing CLI/auth/output surfaces. If Wave 3 is reworked around structured results and the multi-widget invalidation contract is fixed, the phase drops to `MEDIUM`.

---

## Gemini Review

### 1. Summary
The plans for Phase 79 present a comprehensive, well-structured approach to addressing the silent-failure gaps in the `cargo pmcp deploy` process. By splitting the work across 4 logical waves, the plans ensure that schema definitions, pre-build orchestration, post-deploy verification, and CLI/doctor integration are handled systematically. However, there are significant architectural and UX risks—most notably the extreme brittleness of parsing subprocess `stdout` using regex, the dangerous semantics of the `on_failure="rollback"` fallback, and the broken local-development experience caused by the `build.rs` env-var contract.

### 2. Strengths
- **Rigorous Wave Structure:** The separation of configuration/schema (Wave 1) from behavior (Waves 2 & 3) and scaffolding/doctor checks (Wave 4) ensures solid interface-first development.
- **Excellent Threat Modeling:** The plans proactively address path traversal (T-79-02), token leakage in process arguments (T-79-04), and parser DoS (T-79-01) with concrete mitigations.
- **Thoughtful Subprocess UX:** Streaming `stderr` live during `npm build` while capturing `stdout` ensures the developer sees long-running build progress, avoiding the "hanging CLI" illusion.
- **Strong Validation & Testing:** The inclusion of fuzzing for the TOML schema and comprehensive unit/integration test coverage is tightly aligned with the project's high quality standards.

### 3. Concerns
- **HIGH - Brittle Regex Parsing of `stdout`:** The post-deploy verifier's reliance on regex to extract test summaries (e.g., `parse_conformance_summary`) is highly fragile. ANSI color codes, future wording changes in the `test` command, or unexpected warning logs will easily break the extraction.
- **HIGH - `on_failure="rollback"` UX Violation:** Parsing `"rollback"` but silently falling back to `"fail"` (leaving broken code live) after merely printing a warning is dangerous. Users who configure `"rollback"` explicitly expect a rollback; if it fails, they will assume the system reverted and ignore the broken production state.
- **HIGH - `build.rs` Breaks Local Development (`cargo run`):** The `build.rs` template only tracks widget files if `PMCP_WIDGET_DIR` is set. When a developer runs `cargo run` locally (without the deploy wrapper), the env var is missing, falling back to `cargo:rerun-if-changed=Cargo.toml`. This completely breaks the local development loop, as Cargo will cache the `include_str!` and not rebuild when JS changes.
- **MEDIUM - `on_failure="fail"` CI/CD Misinterpretation:** Exiting with a non-zero code while leaving the broken Lambda live will severely confuse CI/CD systems, which universally interpret a non-zero exit from a deploy tool as "the deployment did not happen or was aborted."
- **LOW - Auth Token Resolution in CI:** Reading the OAuth token from `~/.pmcp/oauth-cache.json` works locally, but in CI environments, this cache won't exist. It correctly falls back to `MCP_API_KEY`, but developers must be explicitly warned that CI deployments of OAuth-protected servers require this environment variable to be manually injected.
- **LOW - Dropping `ui/` from Convention:** `ui/` is a ubiquitous convention for frontend code in full-stack repositories. Dropping it forces explicit configuration for many users.

### 4. Suggestions
- **Replace `stdout` Regex with Structured Output:** Instead of regex parsing, implement a `--json` or `--format json` flag on the `cargo pmcp test` commands. The orchestrator can easily deserialize this into strongly-typed results, eliminating brittleness.
- **Hard-Reject `"rollback"`:** If rollback is not implemented, the configuration parser should return a hard error at validation time when `on_failure="rollback"` is encountered, blocking the deployment entirely until the user acknowledges it is unsupported and changes it to `"fail"` or `"warn"`.
- **Fix `build.rs` Local Discovery:** Update the `build.rs` template to attempt local discovery of the widget directory if `PMCP_WIDGET_DIR` is unset (e.g., checking if `widget/` or `widgets/` exists relative to `CARGO_MANIFEST_DIR`). This restores the local development loop for `cargo build` and `cargo run`.
- **Improve CI/CD Clarity:** For `on_failure="fail"`, the CLI should emit a dedicated GitHub Actions/GitLab annotation (e.g., `::error::Deployment succeeded but post-deploy tests failed. Revision is LIVE.`) so that CI surfaces the exact state natively in the UI.
- **Reconsider the Phase Scope:** 12 tasks across 4 waves is massive. Consider splitting this into two distinct phases: Phase 79a (Widget Pre-build) and Phase 79b (Post-deploy Verification). This reduces PR size and PMAT cognitive complexity risks.

### 5. Risk Assessment
**HIGH.**
While the plans are thoroughly detailed, the combination of highly brittle regex parsing for test verification and the dangerous deployment UX regarding rollbacks and CI/CD exit codes presents a high risk of operator confusion, broken production states, and maintenance overhead. Furthermore, the generated `build.rs` script actively breaks the local `cargo run` development loop. These fundamental architectural and UX issues must be addressed before execution.

---

## Consensus Summary

**Both reviewers landed on `HIGH` overall risk** as written. The build half is closer to shippable than the verify half; Wave 3 is the primary risk concentration. Both flagged design mismatches between the plans and the actual current codebase that the plan-checker did not catch (it only inspected plan/research/context files, not the underlying source).

### Agreed Strengths
- **Wave structure and traceability** — both reviewers praised the schema-first → build-half → verify-half → polish ordering and the requirement-to-plan traceability matrix in `79-00-PLAN.md`.
- **Threat modeling depth** — token leakage in `ps` (T-79-04), path traversal (T-79-02), parser DoS (T-79-01), and "live but broken" operator UX are all surfaced in the plans rather than hand-waved.
- **Test planning quality** — fuzzing for TOML schema + comprehensive unit/integration coverage aligned with the repo's quality culture.
- **Embedded_in_crates as explicit source of truth** — both endorsed demoting auto-detection to a doctor hint.

### Agreed Concerns (HIGH consensus — both reviewers HIGH-flagged)

**HIGH-1 — Stdout regex parsing is brittle (CONSENSUS).**
- **Codex framing:** parser path doesn't match current CLI — `--quiet` suppresses output, pretty output comes from `report.rs` not the strings the planner expects.
- **Gemini framing:** ANSI color codes + future wording changes + warning log noise will defeat regex extraction.
- **Both recommend:** structured machine-readable test output (`--format=json` flag, or library-returned `TestReport` struct). This was raised in 79-RESEARCH.md "Open Questions" #1 as the alternative considered and dismissed; both reviewers think dismissing it was the wrong call.

**HIGH-2 — `on_failure="fail"` semantics insufficient for CI/CD (CONSENSUS — Codex HIGH, Gemini MEDIUM).**
- Loud banner helps humans but machine consumers still misinterpret nonzero exit as "deploy did not happen."
- **Both recommend:** dedicated machine-readable signal (unique exit code or JSON status block) distinguishing "deploy succeeded but new revision broken" from "deploy failed before cutover."
- Gemini suggested GitHub Actions/GitLab annotation format (`::error::...`) for native CI surface.

### Codex-only HIGH findings (Codex went deeper into source code than Gemini did)

**HIGH-C1 — Multi-widget cache invalidation is broken.**
- Single `PMCP_WIDGET_DIR` env var + single-dir `build.rs` template = last widget wins when multiple `[[widgets]]` configured. Failure Mode B is NOT solved for multi-widget projects.
- **Recommend:** `PMCP_WIDGET_DIRS` list contract, or per-crate watchers generated from `embedded_in_crates`.

**HIGH-C2 — `resolve_auth_token` fights existing auth refresh path.**
- Current `AuthMethod::None` in `cargo-pmcp/src/commands/auth.rs` already falls back to Phase 74 cache AND transparently refreshes near-expiry tokens.
- The planned parent-side static-bearer-token injection LOSES the refresh behavior, causing false infra failures in CI or under different user accounts.
- **Recommend:** let child subprocesses resolve their own auth via the existing cache/refresh path. Only inject `MCP_API_KEY` when user explicitly supplied it.

### Gemini-only HIGH finding

**HIGH-G1 — `build.rs` template breaks local `cargo run` development loop.**
- When `PMCP_WIDGET_DIR` unset (i.e., developer running `cargo run` directly), template falls back to `cargo:rerun-if-changed=Cargo.toml` only — Cargo caches the `include_str!` and refuses to rebuild on JS changes.
- **Recommend:** when env var unset, attempt local discovery via `CARGO_MANIFEST_DIR + widget/` / `widgets/` lookup. Restores the dev loop without requiring `cargo pmcp deploy` wrapping.

### Gemini-only HIGH finding (UX category)

**HIGH-G2 — `on_failure="rollback"` parse-but-warn-fallback is a UX trap.**
- Operators who explicitly configure `"rollback"` expect rollback. If it silently degrades to `"fail"` after a single warning, they will assume rollback happened and ignore the broken-but-live production state.
- **Recommend:** hard-reject `"rollback"` at config validation time (block deploy entirely until the user changes the value), rather than parse-then-warn-then-treat-as-fail.
- *Tension:* this contradicts the locked CONTEXT.md decision to "reserve the field name without locking out a future addition." Operator should weigh: forward-compat reservation vs. UX-trap risk.

### Codex-only MEDIUM findings worth surfacing

- **Scaffold target misaligned:** current `cargo pmcp app new` template (`cargo-pmcp/src/templates/mcp_app.rs`) uses `WidgetDir` file-serving, NOT `include_str!`. Adding `build.rs` there is mostly harmless but doesn't actually address the stale-binary failure mode for new apps. Doctor scope may need to detect WidgetDir usage explicitly.
- **`build`/`install` override schema underspecified:** whitespace-split breaks quoting (`build = "npm run --silent build"` → wrong argv), and the package.json build-script check is redundant when explicit override given.
- **`node_modules/` heuristic blunt:** Yarn PnP legitimately omits `node_modules` — auto-install fires unnecessarily.

### Divergent Views

- **Convention scope (`ui/` exclusion):** Codex marked LOW (acceptable if docs/doctor point clearly to `[[widgets]]`). Gemini marked LOW with stronger language (`ui/` is "ubiquitous" in full-stack repos, dropping forces explicit config for many users). Both LOW — no action divergence, just emphasis.
- **Phase scope:** Gemini suggested splitting into 79a (build half) + 79b (verify half) due to PR size + PMAT cog risk. Codex more nuanced: only suggested splitting Wave 3 (verify half) if schedule pressure is real. Both agree the verify half is the heavier risk concentration.

### Recommended Next Action

This is a `--reviews`-grade revision opportunity. Given **2 HIGH-CONSENSUS findings** + **3 reviewer-unique HIGH findings** all touching real codebase mismatches:

```
/gsd-plan-phase 79 --reviews
```

The replanner should specifically address:
1. **HIGH-1 (stdout parsing):** decide between (a) splitting work — ship Wave 3 in a follow-on phase after a prerequisite phase adds `--format=json` to the test commands, or (b) staying with regex but treating the metric format as best-effort and degrading gracefully (loses CONTEXT.md verbatim spec compliance for `(8/8 tests passed)`).
2. **HIGH-2 (CI/CD signal):** add unique exit codes (current plan uses 1=test-failed / 2=infra-error — extend with 3=deploy-succeeded-but-revision-broken-and-live) plus GitHub Actions / GitLab annotations.
3. **HIGH-C1 (multi-widget cache):** revise to `PMCP_WIDGET_DIRS` list (colon-separated per Unix `PATH` convention) or per-crate watcher generation.
4. **HIGH-C2 (auth):** strip `resolve_auth_token`; let subprocesses inherit env and resolve via existing `AuthMethod::None` path. Only override when user explicitly provides `--api-key`.
5. **HIGH-G1 (local dev loop):** extend `build.rs` template with local-discovery fallback (`CARGO_MANIFEST_DIR + widget|widgets`) when env var unset.
6. **HIGH-G2 (rollback UX):** operator decision required — reserve the field name (current plan, accept UX-trap risk) OR hard-reject (cleaner UX, requires removing the reservation in CONTEXT.md). Recommend: hard-reject + document upgrade path in 79-04 docs task.

Plus the agreed MEDIUM polish items (scaffold target alignment, build/install argv schema, node_modules heuristic).

### Caveat on Reviewer Coverage

Both reviewers had access to the full plan + research + context but did NOT have access to:
- The repo source code at the same depth (Codex DID inspect specific files via `read_file`; Gemini operated more from the planning artifacts).
- The conversation history that locked CONTEXT.md scope decisions.

Where a reviewer recommendation conflicts with a CONTEXT.md locked decision (e.g., Gemini's hard-reject `rollback` vs. CONTEXT.md's reserve-but-reject), treat the conflict as a flagged decision-revisit, not an automatic override.
