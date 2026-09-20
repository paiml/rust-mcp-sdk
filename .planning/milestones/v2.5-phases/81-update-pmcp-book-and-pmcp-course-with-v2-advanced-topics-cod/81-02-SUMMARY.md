---
phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod
plan: 02
subsystem: docs
tags: [docs, book, code-mode, derive-macro, mdbook, pmcp-code-mode, pmcp-code-mode-derive]

requires:
  - phase: 67.1
    provides: pmcp-code-mode crate (CodeModeConfig, TokenSecret, ValidationPipeline, NoopPolicyEvaluator, CodeExecutor)
  - phase: 67.2
    provides: pmcp-code-mode-derive #[derive(CodeMode)] macro with context_from + language attributes
provides:
  - Rewritten ch12-9-code-mode.md (503 lines, six H2 sections) framed around #[derive(CodeMode)] as canonical happy path
  - Decision tree for direct CodeExecutor impl vs JsCodeExecutor / SdkCodeExecutor / McpCodeExecutor adapters
  - Worked example walkthrough (s41_code_mode_graphql.rs success + rejection paths)
  - Production warning for NoopPolicyEvaluator misuse (T-81-02-03 mitigation)
affects:
  - Phase 81 plan 03 (course ch22 code-mode rewrite — mirror this chapter's pedagogy in course style)
  - Phase 81 plan 07 (book/course build verification audit — verify excerpts vs live example)

tech-stack:
  added: []
  patterns:
    - "Derive-macro-first chapter framing (CONTEXT D-02) — canonical happy path leads, manual handler escape-hatch trails"
    - "Grep-stable inline excerpts (revision R-6) — no hardcoded line numbers, all anchors are grep patterns"
    - "Three-tier reference style: inline excerpts (10-40 lines) + cross-link to full example + table of supported variants"

key-files:
  created: []
  modified:
    - pmcp-book/src/ch12-9-code-mode.md  # 146 → 503 lines (+357, full rewrite)

key-decisions:
  - "Lead with #[derive(CodeMode)] canonical path; demote manual handler registration to 'advanced/escape-hatch'"
  - "Single worked example (s41_code_mode_graphql.rs) end-to-end rather than multiple smaller snippets — pedagogy goal: reader can cargo run --example after the chapter"
  - "Reproduce existing chapter's HMAC pipeline diagram + language→feature table + config.toml block verbatim where they match current crate behavior; rewrite the rest from current API surface"
  - "Cite resolved crate version pins from Cargo.toml (pmcp 2.7, pmcp-code-mode 0.5, pmcp-code-mode-derive 0.2) rather than guessing or using version='*'"
  - "Surface NoopPolicyEvaluator production warning as a blockquote in Policy Evaluation section — T-81-02-03 mitigation"

patterns-established:
  - "Inline excerpt anchor catalog: each excerpt is keyed by a grep pattern in this summary so future drift checks can re-locate them without hardcoded line numbers"

requirements-completed: []

duration: 20min
completed: 2026-05-15
---

# Phase 81 Plan 02: ch12-9 Code Mode chapter rewrite Summary

**Rewrote pmcp-book/src/ch12-9-code-mode.md (146 → 503 lines) around #[derive(CodeMode)] as canonical happy path, with worked round-trip from examples/s41_code_mode_graphql.rs and CodeExecutor decision tree.**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-05-15T20:08:00Z
- **Completed:** 2026-05-15T20:28:00Z
- **Tasks:** 2 (Task 1 discovery, Task 2 rewrite)
- **Files modified:** 1

## Accomplishments

- ch12-9-code-mode.md fully rewritten: 146 lines → 503 lines (+357, +245%)
- All six mandatory H2 sections present in spec order: The Problem, Adding Code Mode with `#[derive(CodeMode)]`, Worked Example, Configuration in `config.toml`, Policy Evaluation, Security Properties Reference
- Derive-macro-first framing enforced — `#[derive(CodeMode)]` is the first substantive section after the problem statement; manual handler registration is explicitly the escape-hatch path
- Crate version pins resolved from Cargo.toml (pmcp 2.7, pmcp-code-mode 0.5.1, pmcp-code-mode-derive 0.2.0) — no guessed versions
- Inline excerpts sourced from `examples/s41_code_mode_graphql.rs` with grep-stable boundaries (catalog below)
- Standard adapters (JsCodeExecutor, SdkCodeExecutor, McpCodeExecutor) framed as a decision tree alongside direct `CodeExecutor` impl
- NoopPolicyEvaluator production warning surfaced as a blockquote in Policy Evaluation section per threat T-81-02-03
- Zero `rust,no_run` blocks (CONTEXT D-10: only the Skills chapter adds a doctest in this phase)
- mdBook build succeeds without errors (`cd pmcp-book && mdbook build`)

## Task Commits

Each task was committed atomically:

1. **Task 1: Read current code-mode crate API surface and derive macro behavior** — no file edits (discovery-only step; verify command confirmed all source files present and derive macro signature stable)
2. **Task 2: Full rewrite of pmcp-book/src/ch12-9-code-mode.md against current API** — `6c3fafb5` (docs)

## Inline Excerpt Source Catalog

Each excerpt in the chapter is keyed by a grep anchor (revision R-6 — NOT hardcoded line numbers). Future drift checks can re-locate excerpts via these patterns:

| Chapter section | Source file | Grep anchor |
|---|---|---|
| § Step 2: Derive and Configure (struct excerpt) | `examples/s41_code_mode_graphql.rs` | `grep -n '#\[derive(CodeMode)\]' examples/s41_code_mode_graphql.rs \| head -1` |
| § Step 2: Derive and Configure (`context_from` snippet) | `pmcp-book/src/ch12-9-code-mode.md` (pre-rewrite reuse) | `grep -n 'fn get_context' pmcp-book/src/ch12-9-code-mode.md \| head -1` |
| § Step 3: Pick a Language (table) | `crates/pmcp-code-mode-derive/src/lib.rs::gen_validation_call` | `grep -n 'gen_validation_call' crates/pmcp-code-mode-derive/src/lib.rs` |
| § Step 4: Direct CodeExecutor impl (GraphQLExecutor impl block) | `examples/s41_code_mode_graphql.rs` | `grep -n 'impl CodeExecutor for GraphQLExecutor' examples/s41_code_mode_graphql.rs \| head -1` |
| § Step 4: Standard adapters (three Arc::new(...) lines) | Synthetic adaptation of pre-rewrite chapter `^### Standard Adapters` block | preceded by `<!-- synthetic -->` HTML comment |
| § Step 5: Register on the Builder (constructor + `register_code_mode_tools`) | `examples/s41_code_mode_graphql.rs` | `grep -n 'register_code_mode_tools' examples/s41_code_mode_graphql.rs \| head -1` |
| § Worked Example: Success path | `examples/s41_code_mode_graphql.rs` | `grep -n -- '--- Success Path' examples/s41_code_mode_graphql.rs \| head -1` |
| § Worked Example: Rejection path | `examples/s41_code_mode_graphql.rs` | `grep -n -- '--- Rejection Path' examples/s41_code_mode_graphql.rs \| head -1` |
| § Configuration in `config.toml` (TOML block) | Pre-rewrite chapter reuse | `grep -n '^## Deployment Configuration' pmcp-book/src/ch12-9-code-mode.md` (pre-rewrite) |
| § Policy Evaluation (PolicyEvaluator trait snippet) | `crates/pmcp-code-mode/src/policy/mod.rs:38` | `grep -n 'pub trait PolicyEvaluator' crates/pmcp-code-mode/src/policy/mod.rs` (preceded by `<!-- synthetic -->` — minimal extracted shape, not the full trait) |
| § How It Works (HMAC pipeline ASCII diagram) | Pre-rewrite chapter reuse | `grep -n '^## How It Works' pmcp-book/src/ch12-9-code-mode.md` (pre-rewrite) |

## Resolved Crate Version Pins

Sourced directly from the workspace Cargo.tomls (Task 1 catalog):

| Crate | Version | Source |
|---|---|---|
| `pmcp` | 2.7.0 | `Cargo.toml::version` |
| `pmcp-code-mode` | 0.5.1 | `crates/pmcp-code-mode/Cargo.toml::version` |
| `pmcp-code-mode-derive` | 0.2.0 | `crates/pmcp-code-mode-derive/Cargo.toml::version` |

The chapter cites these as `pmcp = "2.7"`, `pmcp-code-mode = "0.5"`, `pmcp-code-mode-derive = "0.2"` (caret-style major.minor pins, idiomatic for crates.io).

## `rust,no_run` Block Confirmation

Verified via `! grep -q 'rust,no_run' pmcp-book/src/ch12-9-code-mode.md` — zero matches. The chapter contains only `rust,ignore` Rust blocks and `toml` / `text` blocks. Per CONTEXT D-10, the Skills chapter (plan 81-01) is the sole doctest-bearing chapter in Phase 81.

## Files Created/Modified

- `pmcp-book/src/ch12-9-code-mode.md` — 146 → 503 lines. Replaced with derive-macro-first rewrite covering the full v2 API surface (split-out `pmcp-code-mode` + `pmcp-code-mode-derive` crates, `#[derive(CodeMode)]` macro, language→feature table, CodeExecutor decision tree, worked round-trip).

## Decisions Made

- **Derive-macro-first framing enforced.** `#[derive(CodeMode)]` opens the second H2; manual handler registration is mentioned only as "the advanced/escape-hatch path" in the opening paragraph of that section. This honors CONTEXT D-02 + 81-CONTEXT.md "derive-macro-first framing" specifics.
- **Single worked example.** s41_code_mode_graphql.rs drives the entire `## Worked Example` section. Reader can `cargo run --example s41_code_mode_graphql --features full` after the chapter and see every concept fire.
- **Two synthetic blocks marked.** The standard-adapters snippet and the `PolicyEvaluator` trait shape are pedagogical adaptations rather than verbatim file excerpts; both are preceded by `<!-- synthetic -->` HTML comments so plan 81-07's drift audit (Audit A) will skip them.
- **NoopPolicyEvaluator production warning surfaced.** Blockquote in `## Policy Evaluation` — mitigates T-81-02-03 (chapter risks teaching production-unsafe defaults if NoopPolicyEvaluator caveat is buried).

## Deviations from Plan

None - plan executed exactly as written. All six mandatory H2 sections present in spec order, line count 503 ≥ min 300, all grep anchors resolved against current source.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- ch12-9-code-mode.md is current against the v2 API surface and ready for plan 81-07's mdBook build + drift audit.
- The inline-excerpt source catalog (table above) gives the plan 81-07 verifier a stable index — each excerpt has a grep anchor that re-locates the source span even if `s41_code_mode_graphql.rs` is reformatted.
- The course ch22 rewrite (plan 81-03) can mirror this chapter's pedagogy: derive-macro-first framing, single worked example, CodeExecutor decision tree. The course's distinctive voice lives in its exercises/quizzes (per D-08), not the prose presentation.

## Self-Check: PASSED

- `pmcp-book/src/ch12-9-code-mode.md` exists (503 lines, verified via `wc -l`).
- Task 2 commit `6c3fafb5` exists in git log (`git log --oneline | grep 6c3fafb5`).
- All six mandatory H2 sections grep-confirmed: The Problem, Adding Code Mode with `#[derive(CodeMode)]`, Worked Example, Configuration in `config.toml`, Policy Evaluation, Security Properties Reference.
- Zero `rust,no_run` blocks (CONTEXT D-10 compliance).
- `cd pmcp-book && mdbook build` exits 0 with no errors.

---
*Phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod*
*Plan: 02*
*Completed: 2026-05-15*
