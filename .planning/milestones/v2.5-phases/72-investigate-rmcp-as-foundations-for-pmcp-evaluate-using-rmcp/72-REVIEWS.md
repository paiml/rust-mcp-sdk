---
phase: 72
reviewers: [gemini, codex]
reviewed_at: 2026-04-19T00:00:00Z
plans_reviewed: [72-01-PLAN.md, 72-02-PLAN.md, 72-03-PLAN.md]
---

# Cross-AI Plan Review — Phase 72

## Gemini Review

This review provides an adversarial assessment of the Phase 72 plans for the PMCP SDK foundation evaluation.

### 1. Summary
The strategic framing of Phase 72 is sound, correctly identifying the protocol layer as a commodity that could be delegated to the official `rmcp` crate to focus `pmcp` on its enterprise-DX differentiators (workflows, apps, typed tools). The 5-option strategy matrix and the data-driven decision rubric are robust architectural tools. However, there is a **critical disconnect** between the research intent and the execution plans: while the research mandates executing a "cheap go/no-go" PoC (Slice 1), the plans only cover the creation of the *proposal* and the *rubric*, leaving the final recommendation largely theoretical and "UNRESOLVED."

### 2. Strengths
*   **Inversion Inventory Depth:** The mapping of 29 `pmcp` module families to `rmcp` equivalents is highly detailed, providing a concrete baseline for maintenance-reduction estimates.
*   **Option Granularity:** The five options (A-E) effectively cover the entire spectrum from full adoption to status quo, including a pragmatic "Hybrid" middle ground that aligns with industry precedents like `tonic/prost`.
*   **Falsifiable Rubric:** The use of T-IDs (T1-T7) with explicit data sources (git logs, `gh` CLI, PoC outcomes) prevents the recommendation from being based solely on developer preference.
*   **Risk Awareness:** The plans explicitly track `rmcp` governance responsiveness and `pmcp` user tolerance as "UNRESOLVED" gates, preventing a reckless migration.

### 3. Concerns
*   **PoC Execution Gap (HIGH):** `72-RESEARCH.md` states "Slice 1 is a cheap go/no-go signal... Do it first" and Pitfall 1 explicitly says the phase "executes ≤ 1 PoC slice." However, none of the 3 plans actually execute code. They only write the *72-POC-PROPOSAL.md* document. This means T3 and T4 (the most critical technical gates) will be "UNRESOLVED" in the final recommendation, rendering the phase a "planning to decide" phase rather than a "decision" phase.
*   **Shallow Automated Validation (MEDIUM):** The validation criteria (grep-based row counts) verify that the documents exist and have the right "shape," but they do not verify semantic consistency (e.g., whether the LOC savings in the Matrix match the Inventory).
*   **"UNRESOLVED" Circularity (MEDIUM):** If the recommendation defaults to "Option B (conditional)" because all data gates are unresolved, the phase fails its primary goal of producing an "actionable go/no-go recommendation." It essentially punts the decision to v3.0.
*   **Governance Data Fallback (LOW):** T2 relies on `gh issue list`. If the environment lacks `gh` authentication or network access to that specific org, the "WebFetch" fallback is mentioned but not codified in a way that ensures the median calculation logic is preserved.

### 4. Suggestions
*   **Execute Slice 1 in Plan 02:** Add a task to `72-02-PLAN.md` to actually perform PoC Slice 1 (100 LOC re-export). It is described as a "half-day" task. Running it provides the ground truth for T4 (compile error count) and T1 (LOC delta), allowing Plan 03 to make a data-backed choice between Option A and Option B.
*   **Codify the `gh` Fallback:** In `72-DECISION-RUBRIC.md`, provide the exact `curl` or `web_fetch` URL for the fallback so the "median" data is gathered consistently if the `gh` CLI fails.
*   **Strengthen Cross-Doc Verification:** Add a verification task in Plan 03 that uses a sub-agent (like `gsd-plan-checker`) to specifically verify that the justification in `72-RECOMMENDATION.md` semantically aligns with the thresholds in `72-DECISION-RUBRIC.md`.
*   **Pre-fill CONTEXT.md:** Create a `72-CONTEXT.md` early in Plan 01 to lock the "breaking-change window" (T6) and "user tolerance" (T7) based on known project state, rather than leaving them as "UNRESOLVED."

### 5. Risk Assessment
**Risk Level: MEDIUM**

The plans are architecturally sophisticated and follow the GSD research lifecycle perfectly. The risk is **procedural**: as currently written, the phase will consume significant context to produce five high-quality documents that ultimately say "we need more data." By executing the "cheap" PoC Slice 1 within this phase, the risk drops to LOW and the utility of the recommendation increases significantly.

---

## Codex Review

### Summary

The strategic framing is worth doing: inverting the Phase 69 question from "where does pmcp lag rmcp?" to "what should pmcp stop owning?" is the right architecture question if pmcp wants to differentiate on enterprise DX instead of spec-tracking. The plan is strong on structure, traceability, and scope control, but it is not yet a clean decision framework. It pre-biases the outcome toward `B`, defers several decisive measurements to a future phase while still claiming a final recommendation in Phase 72, and over-relies on weak proxies like LOC deleted, grep gates, and compile-error counts. As written, it is more likely to produce a well-formatted argument for `B` than a genuinely adversarial decision.

### Strengths

- The framing is strategically sound: protocol/wire delegation to `rmcp`, DX differentiation in `pmcp`, is a real and defensible architectural axis.
- The five-option set covers the obvious strategic space better than a binary adopt/stay framing.
- The phase is well-scoped as research-only and explicitly tries to resist migration creep.
- The inversion inventory is the right anchor artifact for this decision; forcing pmcp→rmcp mapping is useful.
- The deliverables are tightly linked, and the phase has better traceability than most research plans.
- The PoC slices target real seams: type identity, handler/service composition, and workflow peer integration.
- Governance risk is at least recognized, which is critical for a foundational dependency decision.

### Concerns

- **HIGH**: The plan wants an evidence-backed final recommendation while leaving core discriminators unresolved. `T3`, `T4`, `T6`, and `T7` are explicitly expected to remain unresolved, and `T2` may also remain unresolved. That means the phase cannot actually distinguish `A` vs `B` vs `D` on its own terms. This conflicts with the falsifiability goal and with `RMCP-EVAL-04`'s "runnable without new data gathering" intent.
- **HIGH**: The plan is biased toward `B` before evidence is gathered. The research already labels `B` "recommended directionally," and Plan 03 explicitly says the default posture under unresolved thresholds is `B (conditional)`. That is a conclusion template, not a neutral decision process.
- **HIGH**: The inventory evidence standard is too weak for a foundation-layer decision. Many rows use `file:line` at `:1`, broad docs.rs module links, and "absence by module listing" as proof. That does not capture type identity, serde shape, trait impl ownership, macro coupling, feature-flag semantics, or runtime assumptions.
- **MEDIUM**: The options are not cleanly distinct. `A` still keeps pmcp-owned types/transports, so it is not really "full adopt." `B` and `A` overlap heavily. `C` is too vague because "selective borrow" can mean types-only, transports-only, or task-manager-only, which are materially different strategies.
- **MEDIUM**: Several rubric thresholds are weak proxies. LOC deleted is not maintenance burden. Median issue close time is not governance reliability. Compile-error count is not SemVer break surface. Conformance percentage is not enterprise feature preservation.
- **MEDIUM**: The PoC slices are useful but insufficiently discriminating. Slice 1 can pass while full adoption is still infeasible. Slice 2 can succeed as a toy adapter while real builder/macro/workflow/client ergonomics still fail. There is no PoC centered on `mcp_apps`, auth/middleware, or streamable HTTP hardening.
- **MEDIUM**: The validation gates are mostly formatting gates. They ensure files exist and tables are filled, not that the content is correct. This is especially risky because the plan instructs executors to copy research tables "verbatim," which reduces the chance of catching bad assumptions.
- **LOW**: There are already signs of consistency drift. For example, the inventory summary says 4 unverified rows, but the detailed inventory names 5 (`auth`, `completable`, `session`, `batch`, `logging`). That is small, but it shows the current gates won't catch semantic mismatch.

### Suggestions

- Add an explicit "insufficient evidence / defer" outcome, or change the phase goal from "final recommendation" to "provisional recommendation plus required next measurements." Without that, the plan forces a pseudo-decision.
- Remove the default-to-`B` logic from Plan 03. If `T2/T3/T4/T6/T7` are unresolved, the process should either defer or conservatively land on `D`, not auto-favor `B`.
- Tighten option definitions:
  - `A`: public foundation shift, rmcp types exposed directly.
  - `B`: internal delegation with pmcp public facade stability as a hard requirement.
  - `C1`: types-only borrow.
  - `C2`: transport-only borrow.
  - `E`: contingency only, not a primary scored strategy.
- Upgrade the inventory schema. Each row should include exact symbols, public API surface impacted, owned impls/macros affected, serde compatibility risk, feature flags, and downstream crates touched.
- Replace or supplement weak thresholds:
  - Use historical churn on `src/types/` and `src/shared/`, not just LOC.
  - Measure maintainer first-response and PR merge latency, not just issue close time.
  - Measure number of public APIs/examples/downstream crates broken, not just compile errors.
  - Measure explicit preservation of `TypedTool`, workflow, `mcp_apps`, auth, middleware, `mcp-preview`, and `cargo-pmcp`.
- Add one more PoC slice around a pmcp-exclusive boundary that matters to the thesis, preferably `mcp_apps` or streamable HTTP/security layering.
- Add a semantic audit gate: independently verify a sample of inventory rows and require the recommendation to cite at least one strongest counterargument against the chosen option.

### Risk Assessment

**HIGH**

The delivery risk is low because this is markdown-only, but the decision-quality risk is high. The plan is structured enough to produce persuasive artifacts without actually resolving the questions that matter most, and it has a built-in tilt toward `B`. If executed as written, it can easily yield a polished but under-validated architectural recommendation.

---

## Consensus Summary

Both reviewers converge on the **same two HIGH-severity findings**: (1) the plan pre-biases the outcome toward Option B, and (2) core discriminating thresholds (T3/T4/T6/T7) are expected to remain UNRESOLVED, which means the phase cannot actually distinguish A vs B vs D on its own terms. Gemini's proposed remedy (execute PoC Slice 1 inside Phase 72) partially addresses both: executing the ~100-LOC Slice 1 resolves T3 and T4, giving the rubric real signal, and reduces the pull toward a default-B outcome. Codex's proposed remedies go further — remove the default-to-B rule entirely, add an explicit "defer" outcome, tighten option definitions (especially split C into C1/C2), and upgrade the inventory evidence standard from `file:line` to exact symbols + serde shape + feature flags + downstream crates touched.

### Agreed Strengths (2+ reviewers)

- **Strategic framing is sound** — inverting Phase 69's question ("what does rmcp lack?") to Phase 72's question ("what should pmcp stop owning?") is the right architecture axis for differentiating on enterprise DX. *(gemini, codex)*
- **Option set covers the space** — five options (A full adopt, B hybrid, C selective borrow, D status quo, E fork) is richer than a binary adopt/stay framing. *(gemini, codex)*
- **Inversion inventory is the right anchor artifact** — forcing a pmcp→rmcp mapping for 29+ module families gives a concrete baseline for maintenance-reduction estimates. *(gemini, codex)*
- **Falsifiability intent via T-IDs + data sources** — threshold IDs with explicit data sources (git log, `gh` CLI, PoC outcomes) prevent preference-driven decisions. *(gemini, codex)*
- **Risk awareness** — governance and user-tolerance gates are tracked as UNRESOLVED rather than silently assumed away. *(gemini, codex)*

### Agreed Concerns (2+ reviewers — highest priority for replan)

- **HIGH — Pre-bias toward Option B.** Plan 03 explicitly defaults to "B (conditional)" when thresholds are unresolved, and the research labels B "recommended directionally" before evidence is gathered. This is a conclusion template, not a neutral decision process. *(gemini "UNRESOLVED Circularity", codex HIGH#2)*
- **HIGH — PoC / threshold resolution gap.** T3 and T4 (and likely T2/T6/T7) are expected to remain UNRESOLVED, so the phase cannot actually distinguish A vs B vs D. Gemini's fix: execute PoC Slice 1 (~100 LOC, half-day) inside Plan 02 to resolve T3/T4. Codex's fix: either change the phase goal to "provisional recommendation + required next measurements," or add an explicit "defer" outcome. *(gemini HIGH "PoC Execution Gap", codex HIGH#1)*
- **MEDIUM — Shallow automated validation.** grep-based row counts verify document shape, not semantic consistency (e.g., LOC savings in the Strategy Matrix match the Inventory; Recommendation justification actually engages the thresholds it cites). *(gemini MEDIUM, codex MEDIUM)*

### Codex-Only HIGH (worth escalating)

- **HIGH — Inventory evidence standard is too weak for a foundation-layer decision.** Many rows use `file:line` at `:1`, broad docs.rs module links, and "absence by module listing" as proof. This does not capture type identity, serde shape, trait impl ownership, macro coupling, feature-flag semantics, or runtime assumptions — all of which are decisive for whether pmcp can actually depend on rmcp's type shapes. Recommendation: upgrade inventory schema to include exact symbols, public API surface impacted, owned impls/macros affected, serde compatibility risk, feature flags, and downstream crates touched.

### Divergent Views

- **Severity of the validation-gate weakness.** Both reviewers flag it, but Codex rates "plans are mostly formatting gates" as MEDIUM while Gemini rates "shallow automated validation" as MEDIUM with a less severe tone. Codex additionally flags this is *especially* risky because plans instruct executors to copy research tables "verbatim," reducing the chance of catching bad assumptions — Gemini does not raise this specific risk.
- **Overall risk level.** Gemini: **MEDIUM** ("procedural risk; drops to LOW if Slice 1 executes"). Codex: **HIGH** ("decision-quality risk is high; can yield persuasive but under-validated recommendation"). The divergence is meaningful: Gemini believes one surgical addition (execute Slice 1) fixes the plan; Codex believes the plan has deeper structural biases (default-to-B, weak options, weak thresholds, weak evidence) that require broader restructuring.
- **Option structure.** Codex wants C split into C1 (types-only borrow) and C2 (transports-only borrow) because they are "materially different strategies"; Gemini accepts the 5-option set as-is and calls option granularity a strength.

### Recommended Replan Actions (to feed into `/gsd-plan-phase 72 --reviews`)

1. **Remove the default-to-B rule from Plan 03.** On UNRESOLVED thresholds, either defer explicitly or land on the most conservative option (D), not B. *(both reviewers)*
2. **Execute PoC Slice 1 inside Plan 02** (new task: ~100 LOC re-export experiment, half-day time-box, deletes on completion). Resolves T3 and T4 with real data. *(gemini)*
3. **Pre-fill CONTEXT.md early in Plan 01** to lock T6 (breaking-change window) and T7 (user tolerance) from known project state. *(gemini)*
4. **Upgrade inventory row schema** to include exact symbols, public API surface, impls/macros owned, serde compat risk, feature flags, downstream crates touched. *(codex)*
5. **Replace weak proxies in the rubric**: historical churn on `src/types/` and `src/shared/` (not just LOC); PR merge latency (not just issue close time); public-API/example/downstream-crate break count (not just compile errors); explicit enterprise-feature preservation check for `TypedTool`, workflow, `mcp_apps`, auth, middleware, `mcp-preview`, `cargo-pmcp`. *(codex)*
6. **Add a semantic audit gate** in Plan 03 (cross-doc consistency beyond grep: every `72-RECOMMENDATION.md` criterion subsection cites a specific threshold outcome and supporting inventory/matrix row). *(both reviewers)*
7. **Consider splitting option C** into C1 (types-only) and C2 (transports-only); demote E to "contingency only, not primary scored strategy." *(codex)*
8. **Codify the `gh` fallback** in `72-DECISION-RUBRIC.md` with an exact WebFetch URL so the median calculation is gathered consistently if `gh` fails. *(gemini)*
