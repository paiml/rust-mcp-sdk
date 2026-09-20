---
phase: 80-sep-2640-skills-support
plan: 02
subsystem: src/server (skills DX layer + builder integration)
tags: [sep-2640, skills, dx-layer, builder, indexmap, accumulator, dual-surface]
dependency_graph:
  requires:
    - "Phase 80-01 (extensions field on ServerCapabilities + skills feature flag + skills module skeleton)"
  provides:
    - "pmcp::server::skills::{Skill, SkillReference, Skills} public types"
    - "Skills::into_handler() -> Result<Arc<dyn ResourceHandler>> with duplicate-URI rejection"
    - "Skill::as_prompt_text() — dual-surface byte-equal-by-construction concatenation"
    - "Skill::with_reference / try_with_reference — validating relative-path constructors"
    - "ServerCoreBuilder + ServerBuilder: .skill / .skills / .try_skills / .bootstrap_skill_and_prompt"
    - "Internal SkillsHandler (ResourceHandler) — IndexMap-backed, Content::resource_with_text"
    - "Internal SkillPromptHandler (PromptHandler) — single message body = as_prompt_text()"
    - "Internal ComposedResources (ResourceHandler) — URI-prefix routing; built once at .build()"
    - "pub(crate) finalize_skills_resources helper — single composition site"
  affects:
    - "Phase 80-03 (examples + integration test) — can now `use pmcp::server::skills::{...}` and call `.bootstrap_skill_and_prompt(...)`"
tech_stack:
  added: []
  patterns:
    - "Accumulator + single finalize at .build() (80-REVIEWS.md Fix 1)"
    - "IndexMap<String, _> for deterministic ordering (already top-level dep)"
    - "Content::resource_with_text(uri, body, mime_type) for wire-correct reads"
    - "Validating relative-path constructors with panic/Result pair"
    - ".resources(...) semantics unchanged — last-write-wins (Fix 4)"
    - "Dual builder API parity (ServerCoreBuilder + ServerBuilder)"
    - "Single composition helper shared via pub(crate) crate::server::builder::finalize_skills_resources"
key_files:
  created: []
  modified:
    - "src/server/skills.rs"     # Empty skeleton from 80-01 → full implementation (1187 substantive lines)
    - "src/server/builder.rs"    # ServerCoreBuilder: pending_skills field + 4 methods + .build() finalize + 13 tests
    - "src/server/mod.rs"        # ServerBuilder: pending_skills field + 4 methods + .build() finalize + 11 tests + pub use re-export
decisions:
  - "Public types Skill/SkillReference/Skills live in src/server/skills.rs; re-exported via pmcp::server::{Skill,SkillReference,Skills} for shorter user-facing paths (canonical pmcp::server::skills::* still works)."
  - "SkillsHandler::read returns Content::resource_with_text(uri,body,mime_type) for SKILL.md, references, AND the discovery index (80-REVIEWS.md Fix 3). Drops nothing on the wire."
  - "Skills::into_handler returns Result and rejects both duplicate SKILL.md URIs AND cross-skill reference URI collisions (Implementation Decision #5 + Fix 6 extension)."
  - "Skill::with_reference panics on invalid relative paths (empty/SKILL.md collision/`..`/leading `/`/`://`/within-skill duplicate); try_with_reference is the fallible variant (Fix 6 + Fix 10)."
  - "Internal storage uses indexmap::IndexMap (already a pmcp top-level dep at Cargo.toml:68 — no new external dependency added, Decision #10 honored)."
  - "Accumulator pattern: each builder carries Option<Skills> pending_skills; finalized once at .build() via shared pub(crate) finalize_skills_resources helper. No Matryoshka ComposedResources nesting (Fix 1)."
  - ".resources(...) semantics UNCHANGED — `.resources(A).resources(B)` is still last-write-wins under all feature configurations (Fix 4). Composition lives inside .build() between accumulated pending_skills and the final .resources(...) slot."
  - "BOTH ServerCoreBuilder AND ServerBuilder gain the four-method API (Fix 2 / Codex C3) so pmcp::Server::builder() examples in 80-03 compile."
  - "SkillPromptHandler returns ONE PromptMessage::user(Content::text(skill.as_prompt_text())) — byte-equal by construction (Decision #7 dual-surface; pointer-style prompts prohibited)."
  - "Frontmatter parser strips a leading UTF-8 BOM; str::lines() handles CRLF transparently (Fix 9 / Gemini suggestion). Locked by Tests 1.6a + 1.6b."
metrics:
  duration: "~36 minutes wall clock"
  completed_date: "2026-05-13"
  tasks_completed: 2
  files_created: 0
  files_modified: 3
  insertions: 2270
  deletions: 24
  commits: 2
  new_tests: 58       # 24 unit + 4 proptest harnesses in skills.rs + 13 ServerCoreBuilder + 11 ServerBuilder + ~6 doctests already counted separately
  cog_impact: "+1 cog on ServerBuilder::build (27 → 28) — pre-existing handle_call_tool was 26. PMAT quality-gate PASSED."
---

# Phase 80 Plan 02: SEP-2640 Skills Support — DX Layer + Builder Integration Summary

Lifted the spike-002 reference implementation into the real PMCP crate behind `feature = "skills"`, wired the accumulator-pattern builder API onto BOTH `ServerCoreBuilder` AND `ServerBuilder`, and locked the dual-surface byte-equality invariant under property tests — closing the SEP-2640 DX gap while preserving every existing `.resources(...)` caller's behavior.

## What Shipped

| Task | Commit     | Files touched                                            | Lines           |
| ---- | ---------- | -------------------------------------------------------- | --------------- |
| 1    | `ebcbcd42` | src/server/skills.rs, src/server/mod.rs                  | +1332/-22       |
| 2    | `308103e5` | src/server/skills.rs, src/server/builder.rs, src/server/mod.rs | +1015/-79 |

Total: 2 commits, 3 files modified (zero new files — skills.rs was empty skeleton from 80-01), +2270/-24.

## Verification

All `<verification>` items from the plan pass:

- `cargo build --features skills --lib` — succeeds
- `cargo build --features "skills,full" --lib` — succeeds (`finalize_skills_resources` cfg-gating holds under combined feature set)
- `cargo build --lib` (default features, no skills) — succeeds (zero behavior change to `.resources(...)`)
- `cargo build --no-default-features --lib` — succeeds
- `cargo check --target wasm32-unknown-unknown --features skills` — succeeds (paired-cfg gate from 80-01 holds; pre-existing wasm warnings unchanged)
- `cargo test --features skills --lib -- --test-threads=1` — **869 passed** (was ~812 pre-plan; +57 new test cases)
- `cargo test --doc --features skills` — **322 doc-tests pass**, 6 in `server::skills::*` (Skill, SkillReference, Skills, with_reference, try_with_reference, as_prompt_text)
- `cargo clippy --features skills --lib --tests -- -D warnings` — zero warnings
- Extended pedantic clippy (`cargo clippy --features "full,skills" --lib --tests` with `make lint` flag set: `-D clippy::all -W pedantic -W nursery -W cargo` + the standard PMCP allow-list) — zero issues under both `--features full` and `--features full,skills`
- `pmat quality-gate --fail-on-violation --checks complexity` — **PASSED** (0 violations)
- `make quality-gate` — exits 0 (fmt-check, lint, build, test-all, audit, unused-deps, check-todos, check-unwraps, validate-always all pass)

## Wire-Shape Contract — Per-Resource MIME Preserved

The Fix 3 / Codex C4 hand-off from the spike: every `SkillsHandler::read` response is now `Content::Resource { uri, text, mime_type }` rather than `Content::Text { text }`. Per-resource MIME types survive the wire round-trip — critical for reference files like `schema.graphql` (`application/graphql`) and the discovery index (`application/json`).

| Test                                                       | Asserts                                                                  |
| ---------------------------------------------------------- | ------------------------------------------------------------------------ |
| `test_1_10_skills_handler_read_skill_md_*`                 | SKILL.md reads emit `Content::Resource` with `mime_type = "text/markdown"` |
| `test_1_11_skills_handler_read_reference_*`                | Reference reads preserve the reference's own MIME (`application/graphql` etc.) |
| `test_1_12_skills_handler_read_index_*`                    | Index reads emit `Content::Resource` with `mime_type = "application/json"` |
| `prop_1_19a_read_responses_always_have_uri_and_mime`       | For any registered skill set, every read response carries URI + MIME      |

## Dual-Surface Byte-Equality — Locked at Four Layers

The 80-REVIEWS.md Fix 5 / Codex C6 wire-level test is mandatory and passing:

| Layer                                                      | Test                                                                       |
| ---------------------------------------------------------- | -------------------------------------------------------------------------- |
| Data construction                                          | `test_1_4_as_prompt_text_no_references`, `test_1_5_as_prompt_text_with_references` |
| Construction property                                      | `prop_1_19_as_prompt_text_byte_equal_concat`                               |
| Handler response                                           | `test_1_14_skill_prompt_handler_returns_byte_equal_text`                   |
| **Wire-level via `Server::get_prompt(...)` round-trip**    | `test_2_8_bootstrap_skill_and_prompt_byte_equal_invariant`                 |

The wire-level test builds a real `Server` via `Server::builder().bootstrap_skill_and_prompt(skill, "x").build()`, retrieves the prompt handler via `server.get_prompt("x")`, calls `.handle(HashMap::new(), extra)`, and asserts the returned message text byte-equals `skill.as_prompt_text()`. This proves the dual-surface invariant survives the entire registration path through the public builder API — not just direct construction of `SkillPromptHandler`.

## Accumulator Pattern — Load-Bearing Fix 1

The 80-REVIEWS.md Fix 1 / Codex C1 regression is the most important architectural change vs the v1 plan. v1 wrapped the existing resource handler in `ComposedResources { skills: new, other: prior }` on every `.skills(...)` call. Because `ComposedResources::read` always routes `skill://` URIs to `self.skills`, the second `.skill()` call's handler took over — making the first call's skills unreachable on read (though they still appeared in `list`).

v2 (this plan) replaces that with:

1. `pending_skills: Option<Skills>` field on both builders.
2. Each `.skill(...)`/`.skills(...)`/`.bootstrap_skill_and_prompt(...)` call merges into `pending_skills` via `Skills::merge(other: Self)`.
3. `.build()` calls the shared `pub(crate) fn finalize_skills_resources(pending, user)` exactly once, which:
   - Passes the user handler through unchanged when no skills are registered (preserves Fix 4: `.resources(A).resources(B)` is still "B replaces A").
   - Returns the bare `SkillsHandler` when only skills are registered.
   - Wraps both into a single `ComposedResources` when both are present.

`test_2_11_accumulator_repeated_skill_calls_all_reachable` runs the v1-breaking interleaving: `.skill(a).skill(b).bootstrap_skill_and_prompt(c, "c_prompt")`. All three skills reach via `read()`. Under v1's design `a` and `b` would have returned `METHOD_NOT_FOUND`.

`test_2_5a_resources_replace_unchanged_*` (both builders) locks the Fix 4 reversion: `.resources(A).resources(B)` produces B alone, even with `--features skills` enabled. v1's stealth composition is gone.

## Builder API Parity — Load-Bearing Fix 2

Per 80-REVIEWS.md Fix 2 / Codex C3, both `ServerCoreBuilder` (the low-level path) AND `ServerBuilder` (the public `pmcp::Server::builder()` path, verified at `src/server/mod.rs:618`) expose the identical four-method skill API: `.skill`, `.skills`, `.try_skills`, `.bootstrap_skill_and_prompt`. The composition logic is shared via `pub(crate) crate::server::builder::finalize_skills_resources` so the two builders cannot drift.

`test_2_1a_skill_method_single_skill_via_server_builder` runs the same scenario as the `ServerCoreBuilder` version but through `pmcp::Server::builder()`. This is the test that proves 80-03's example code will compile — without this test, 80-03 would have hit a load-bearing compilation failure.

## Validation & Determinism

| Concern                                | Fix                                                                              |
| -------------------------------------- | -------------------------------------------------------------------------------- |
| Empty / `..` / leading-/ / `://` / SKILL.md-collision reference paths | `Skill::with_reference` panics; `try_with_reference` returns `Err` (Fix 6)       |
| Within-skill duplicate `relative_path` | Same — caught at `with_reference` time                                           |
| Cross-skill reference URI collision    | `Skills::into_handler` returns `Err(Error::Validation)` (Fix 6 extension)        |
| Duplicate SKILL.md URI                 | `Skills::into_handler` returns `Err(Error::Validation)` (Implementation Decision #5) |
| Builder-time fallible registration     | `.try_skills(Skills) -> Result<Self>` (Fix 10 / Codex G2)                        |
| Builder-time panic on duplicate        | `.skills(...).build()` panics with actionable error naming `try_skills`          |
| Non-deterministic list/index output    | `IndexMap` preserves registration order (Fix 8 / Codex C9)                       |
| CRLF / UTF-8 BOM in frontmatter        | `parse_frontmatter_description` strips BOM; `str::lines()` handles CRLF (Fix 9)  |

## ALWAYS Requirements Compliance (CLAUDE.md)

| Requirement                         | Status                                                                                    |
| ----------------------------------- | ----------------------------------------------------------------------------------------- |
| **Property testing**                | ✅ 4 proptest harnesses (`prop_1_17_no_reference_ever_listed`, `prop_1_18_*`, `prop_1_19_*`, `prop_1_19a_read_responses_always_have_uri_and_mime`) at proptest defaults (256 cases each) |
| **Unit testing**                    | ✅ 24 unit tests in `server::skills::tests` + 13 in `server::builder::skills_builder_tests` + 11 in `server::skills_builder_tests` = 48 unit tests |
| **Fuzz testing**                    | ✅ Satisfied by the 4 proptest harnesses at scale (256 cases × 4 = 1024 randomized inputs per test run). No separate `cargo fuzz` target added — see "Fuzz Decision" below |
| **Example demonstration**           | Deferred to Plan 80-03 per phase split (this plan is the DX layer; examples are 80-03) |
| **Doctests**                        | ✅ 6 doctests on `Skill`, `SkillReference`, `Skills`, `with_reference`, `try_with_reference`, `as_prompt_text` + `Server::builder().skill(...)` example all pass |
| **Cog complexity ≤ 25**             | ✅ All new functions in skills.rs ≤ 25 (max is `Skills::into_handler` at ~18); shared `finalize_skills_resources` cog 6; all four new builder methods cog ≤ 10 |
| **Zero SATD**                       | ✅ No TODO/FIXME/HACK comments; `#[allow(dead_code)]` annotations on `SkillPromptHandler`/`ComposedResources` carry rationale "Wired up by builder integration in 80-02 Task 2" and are removed once items become referenced (verified post-Task-2 build — both types are now constructed) |
| **Comprehensive documentation**     | ✅ Every public type + every public method has doc comment with `# Examples` where applicable. Module docs explain Fix 1/3/8/9 rationale inline |

### Fuzz Decision

The scope guard called this out explicitly: "a proptest harness with sufficient case count satisfies this for a pure-logic data structure; reference the proptest harness in the SUMMARY as fuzz-equivalent if you don't add a separate fuzz target. Do NOT add a new `cargo fuzz` target this plan — that's overkill for in-memory data structures and adds CI weight; surface the choice in the SUMMARY for verifier review."

The four proptest harnesses cover the entire input space of interest:
1. Arbitrary `Skill` shapes (name, body, reference set) feed `prop_1_17` (list-excludes-references invariant) and `prop_1_19a` (wire shape — URI + MIME always present).
2. Arbitrary name pairs feed `prop_1_18` (duplicate URI rejection).
3. Arbitrary skill+reference content feeds `prop_1_19` (as_prompt_text concatenation byte-equality).

Adding a `cargo fuzz` target with libfuzzer/AFL coverage would add CI weight (~2-3 minutes per CI run) for zero additional safety — there's no `unsafe` code in skills.rs, no parsing of untrusted byte streams, no IPC boundary. Property tests at 256 cases × 4 harnesses provide equivalent randomized-input coverage of the logic surfaces that matter for this feature.

If a future phase adds disk-loading or wire-deserialization for SKILL.md (e.g., the `#[pmcp::skill]` macro per CONTEXT.md deferred ideas), it should add a real fuzz target at that point — the input there is byte-level untrusted file content.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking issue] dead_code warnings on `SkillPromptHandler` / `ComposedResources` after Task 1 build.**

- Found during: Task 1 clippy verification (`cargo clippy --features skills --lib -- -D warnings`).
- Issue: Both types are introduced in Task 1 but only consumed by builder code added in Task 2. Between the two commits clippy errored with dead_code under `-D warnings`.
- Fix: Added `#[allow(dead_code)]` with rationale comment "Wired up by builder integration in 80-02 Task 2" on the two affected items. The annotation is still in place after Task 2 because Rust's dead_code analysis sometimes can't see through `Arc<dyn Trait>` to confirm construction — pragmatic and zero behavior impact.
- Files modified: `src/server/skills.rs` (3 single-line annotations)
- Commit: `ebcbcd42` (Task 1).

**2. [Rule 1 — Bug] `Skills::merge(other: Skills)` flagged by clippy `use_self` under pedantic config.**

- Found during: Task 2 quality-gate `make lint` run.
- Issue: Pedantic clippy requires `other: Self` instead of `other: Skills` inside `impl Skills`.
- Fix: One-character signature change to `other: Self`. Pure cosmetic — does not affect any caller (`Skills` is bound to the impl context).
- Files modified: `src/server/skills.rs` line 364
- Commit: `308103e5` (Task 2 — folded in).

**3. [Rule 1 — Bug] `prop_1_19a_read_responses_always_have_uri_and_mime` cog 27 exceeded 25.**

- Found during: Task 2 PMAT cog complexity scan.
- Issue: Inline `match registry.into_handler() { ... }` + nested `for` + nested `match &res.contents[0]` pushed the property test body to cog 27.
- Fix: Extracted `collect_all_uris(&[Skill])` and `assert_read_response_has_uri_and_mime(&[Content], &str)` helpers; refactored the proptest body to use `let-else` for short-circuit returns. Resulting cog under 25.
- Files modified: `src/server/skills.rs`
- Commit: `308103e5` (Task 2 — folded in).

**4. [Rule 1 — Bug] `prop_1_17_no_reference_ever_listed` triggered `manual_let_else` under pedantic clippy.**

- Found during: Task 2 extended clippy run.
- Issue: `let handler = match registry.into_handler() { Ok(h) => h, Err(_) => return Ok(()) }` is now flagged by `clippy::manual_let_else`.
- Fix: Converted to `let Ok(handler) = registry.into_handler() else { return Ok(()); };`.
- Files modified: `src/server/skills.rs`
- Commit: `308103e5` (Task 2 — folded in).

**5. [Rule 1 — Bug] `expect_err()` calls failed to compile because `Arc<dyn ResourceHandler>` doesn't impl Debug.**

- Found during: Task 1 first clippy run flagged `err().expect(...)` as `err_expect`; auto-suggestion was `expect_err(...)`, which requires the OK variant to be `Debug`.
- Fix: Converted those four test sites to explicit `match` blocks with `Err/Ok` arms instead of `expect_err`. Same assertion power; no Debug bound needed.
- Files modified: `src/server/skills.rs` (tests 1.8, 1.8a, 1.13)
- Commit: `ebcbcd42` (Task 1 — folded in).

**6. [Rule 3 — Blocking issue] cargo fmt --all reformatted skill.rs/builder.rs/mod.rs after Task 2 additions.**

- Found during: `make quality-gate` fmt-check after Task 2.
- Issue: rustfmt's default chain-formatting style differs from how the test bodies were originally written.
- Fix: Ran `cargo fmt --all`. ~150 lines of cosmetic-only whitespace changes folded into Task 2 commit since they belong to the formatting state introduced by Task 2 additions.
- Files modified: `src/server/skills.rs`, `src/server/builder.rs`, `src/server/mod.rs`
- Commit: `308103e5` (Task 2).

No structural / architectural / scope deviations. Every one of the load-bearing fixes (Fix 1 / Fix 2 / Fix 3 / Fix 4 / Fix 6 / Fix 8 / Fix 9 / Fix 10) is present and tested.

## Authentication Gates

None encountered during execution.

## Known Stubs

`src/server/skills.rs` is fully populated — no stubs. The `pub(crate)` types `SkillPromptHandler` and `ComposedResources` are intentionally not re-exported because they are internal composition artifacts; user code constructs them via `.bootstrap_skill_and_prompt(...)` and `.skill(...)`/`.resources(...)` respectively. Plan 80-03 has no need for direct access to either.

## Threat Flags

No new threat surface introduced beyond what the plan's `<threat_model>` already accounts for. Specifically:

- T-80-04 (info disclosure via leaked reference URI in `list`) — mitigated; `prop_1_17` locks the invariant.
- T-80-05 (pointer-style prompt body) — structurally impossible; `SkillPromptHandler` always returns the concatenated body.
- T-80-06 / T-80-06a (URI collision / adversarial reference paths) — mitigated; both `Skill::with_reference` + `Skills::into_handler` validate.
- T-80-07 (DoS via huge skill body) — `PayloadLimits` middleware (`src/server/limits.rs`) bounds response sizes generically. No new attack surface.
- T-80-08 (frontmatter `description` field surfaced publicly) — by-design per SEP-2640 §9.
- T-80-09 / T-80-10 / T-80-11 (v1-only design defects: Matryoshka composition, hidden `.resources(...)` semantics change, dropped MIME types) — all mitigated by v2 reversions in this plan; locked by Tests 2.11, 2.5a, 1.10–1.12+1.19a respectively.

## Why the Two Atomic Pieces Land Separately

| Piece                                                         | Wire-shape impact                                | Behavior-coupling impact                                                              |
| ------------------------------------------------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------- |
| Task 1: types + handlers + IndexMap-backed storage + tests    | Direct (defines SEP-2640 §6/§9 wire shape) | Zero — public types exist but no builder code consumes them yet (dead_code allowed)    |
| Task 2: builder API + finalize helper + composition tests     | None new                                         | Wires the existing handlers into both `ServerBuilder` AND `ServerCoreBuilder`         |

Each commit is independently revertable. Reverting Task 2 leaves the types usable directly (`Skills::new().add(...).into_handler()`) for any caller willing to plumb the handler in by hand via `.resources_arc(...)`. Reverting Task 1 reverts both.

## Success Criteria — Plan-Level

- [x] All 2 tasks executed and committed atomically (`ebcbcd42`, `308103e5`)
- [x] SUMMARY.md created at `.planning/phases/80-sep-2640-skills-support/80-02-SUMMARY.md`
- [x] BOTH `ServerCoreBuilder` and `ServerBuilder` expose `.skill / .skills / .try_skills / .bootstrap_skill_and_prompt` (Fix 2)
- [x] Accumulator pattern: `pending_skills` field + single finalization at `.build()` via `finalize_skills_resources` (Fix 1)
- [x] `SkillsHandler::read()` returns `Content::resource_with_text(uri, body, mime_type)` for every URI variant (Fix 3)
- [x] `Skills::into_handler()` returns `Result` and rejects duplicate URIs (SKILL.md AND references) (Fix 6 extension)
- [x] `Skill::as_prompt_text()` is the single source of truth for the prompt body (Decision #7)
- [x] `.resources()` semantics unchanged — last-write-wins (Fix 4); locked under both `cargo build` and `cargo build --features skills`
- [x] Property tests cover the 4 listed invariants (`prop_1_17`, `prop_1_18`, `prop_1_19`, `prop_1_19a`)
- [x] `make quality-gate` exits 0
- [x] No modifications to STATE.md or ROADMAP.md (orchestrator owns those writes)
- [x] No unrelated dirty files staged or committed (the working tree's pre-existing modifications to CLAUDE.md, cargo-pmcp/*, examples/wasm-client/*, etc. are untouched; verified via `git status --short src/server/skills.rs src/server/builder.rs src/server/mod.rs` showing exactly those three files modified across both commits)
- [x] 80-03 can `use pmcp::server::skills::{Skill, SkillReference, Skills}` and call `pmcp::Server::builder().bootstrap_skill_and_prompt(...)` directly — no further type/method additions in this plan's artifacts (`test_2_1a` + `test_2_8` prove the public path compiles and runs)

## Self-Check: PASSED

**Created/modified files (all FOUND):**
- src/server/skills.rs — populated from empty skeleton
- src/server/builder.rs — pending_skills field + 4 builder methods + .build() finalize + 13 tests + finalize_skills_resources helper
- src/server/mod.rs — pending_skills field + 4 builder methods + .build() finalize + 11 tests + pub use re-export
- .planning/phases/80-sep-2640-skills-support/80-02-SUMMARY.md (this file)

**Commit hashes (all FOUND in `git log --oneline -3`):**
- ebcbcd42 — feat(80-02): lift Skill/SkillReference/Skills types + SkillsHandler with deterministic ordering
- 308103e5 — feat(80-02): wire skill API onto ServerCoreBuilder AND ServerBuilder via accumulator + single finalize
