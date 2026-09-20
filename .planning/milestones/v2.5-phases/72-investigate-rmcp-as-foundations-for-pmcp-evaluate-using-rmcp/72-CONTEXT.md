# Phase 72 — Context Locks (T6, T7)

**Purpose:** lock the two rubric thresholds that cannot be derived from code analysis or documentation: T6 (pmcp v2.x breaking-change window) and T7 (production-user tolerance for v3.0). This file is the source of truth for those two inputs into `72-DECISION-RUBRIC.md`.

**Why this file exists (per 72-REVIEWS.md replan action #3):** without explicit locks here, T6 and T7 become permanently UNRESOLVED in the decision rubric, which forces the recommendation toward DEFER for reasons unrelated to the evidence the phase actually gathered.

**Rule:** every field below is either a concrete value or `UNKNOWN`. If `UNKNOWN`, a "Resolution path" MUST follow. Leaving fields blank is forbidden.

---

## T6 — pmcp v2.x breaking-change window

breaking_change_window: UNKNOWN

**Evidence cited:**
- `MEMORY.md` — "v2.0 cleanup philosophy" feedback entry: *"During breaking-change window, consolidate aggressively — don't defer as 'not worth the churn'"* — this indicates the window was open at least during the v2.0 cleanup work, but does NOT tell us whether it is still open in v2.4.0 as of 2026-04-19.
- Current pmcp version from root `Cargo.toml:3` → `version = "2.4.0"` (SemVer pre-v3.0 territory — a breaking change would require v3.0, not a further v2.x bump).
- `STATE.md` milestone is v2.0 "Protocol Modernization" (complete) / v2.1 rmcp Upgrades (executing). No explicit SemVer policy text in `CLAUDE.md` or `PROJECT.md` states whether the v2.x window is formally closed.
- Recent Phase 70 threat register entry "RequestHandlerExtra is now `#[non_exhaustive]`. This is a **breaking change**" (from `src/server/cancellation.rs` doc comment lines 19–22) shipped inside the v2.x series, suggesting the window was treated as *open* as recently as Phase 70 — but a single breaking change absorbed under `#[non_exhaustive]` is not proof the window is still open for arbitrary breakage under Option A/B of this phase.

**Resolution path (if UNKNOWN):** ask user directly — "Is the pmcp v2.x breaking-change window still open? If closed, provide the close date." This is a binary question and does not require code analysis. The /gsd-verify-work step for Plan 03 is the expected point at which this is resolved before final recommendation.

**Implication for Plan 03 recommendation:**
- If `open`: Option A, B, C1, C2 all remain eligible (subject to other thresholds).
- If `closed`: Options A and B are disqualified; Plan 03 must pick among C1, C2, D, or DEFER.
- If `UNKNOWN`: Plan 03 lists T6 as UNRESOLVED and counts it against the decision-tree resolved-threshold total.

---

## T7 — Production-user tolerance for v3.0

production_user_tolerance: UNKNOWN

**Evidence cited:**
- `MEMORY.md` — `pmcp-run` is named as "a confirmed major consumer" of pmcp (verified downstream production user, count ≥ 1).
- `MEMORY.md` → feedback index references `feedback_lambda_dns_rebinding.md` — the existence of a specific Lambda DNS-rebinding bypass feedback entry implies ≥ 1 Lambda production deployment filed the issue.
- No crates.io reverse-dependency survey has been run (would require `cargo search`/`crates.io/dependents` query not executed in this plan).
- No `pmcp` public forum or Discord census is available in repo state.
- Lower bound from repo evidence alone: **≥ 2 production deployments** (pmcp-run + ≥ 1 Lambda consumer). This is a floor, not a ceiling — actual count is unknown.

**Resolution path (if UNKNOWN):** either (a) ask user for a count and a go/no-go on v3.0 breaking changes, or (b) run a lightweight pmcp-user survey (post in pmcp discussion forum / crates.io dependents listing / pmcp-run team direct ask). Option (a) is the expected /gsd-verify-work resolution for Plan 03.

**Tolerance interpretation:**
- If production_user_tolerance = 0 OR 1-2 with explicit "no v3.0": Option A/B disqualified → prefer D.
- If 3-5 with ≥ 2 tolerating: Option A or B eligible.
- If 6-20 or 21+: Plan 03 MUST add a user-communication subtask to Next Steps regardless of option.
- If UNKNOWN: Plan 03 lists T7 as UNRESOLVED.

---

## Other Context (informational, not rubric-gating)

- **Baseline pin:** rmcp 1.5.0 (inherited from Phase 69 D-02) / pmcp 2.4.0 (from `Cargo.toml:3`).
- **Phase 72 is research-only.** No rmcp dependency is added to pmcp in this phase. Plan 02 executes a throwaway spike on a scratch branch that is deleted before any commit.
- **Contingency option E (Fork rmcp):** NOT a primary scored strategy per 72-REVIEWS.md replan action #7. Documented in 72-STRATEGY-MATRIX.md as a footnote only.

---

## Closes

T6 lock (explicit UNKNOWN + Resolution path) and T7 lock (explicit UNKNOWN + Resolution path) for use by 72-DECISION-RUBRIC.md. Both resolve via user input during /gsd-verify-work for Plan 03; the resolution paths above name the exact questions to ask.
