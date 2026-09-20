---
phase: 80-sep-2640-skills-support
plan: 03
subsystem: examples + tests (SEP-2640 skills usage + integration coverage)
tags: [sep-2640, skills, examples, integration-test, dual-surface, fix-3, fix-5, fix-9]
dependency_graph:
  requires:
    - "Phase 80-01 (extensions field on ServerCapabilities + skills feature flag + module skeleton)"
    - "Phase 80-02 (Skill/SkillReference/Skills types + ServerBuilder API + SkillsHandler wire shape + dual-surface PromptHandler)"
  provides:
    - "examples/s44_server_skills.rs — public-builder three-tier skill registration demo"
    - "examples/c10_client_skills.rs — both-host-flows client demo with byte-equality + Content::Resource wire-shape assertions"
    - "examples/skills/ tree with spike-002-byte-equal SKILL.md + reference bodies"
    - "tests/skills_integration.rs — 4 SEP-2640 endpoints + mandatory wire-level dual-surface (Fix 5) + CRLF (Fix 9) + 2 proptest harnesses"
  affects:
    - "End users: cargo run --example s44_server_skills/c10_client_skills documents the SEP-2640 usage story end-to-end"
    - "Regression coverage: 10-test integration suite + 2 proptest harnesses fail loudly if the wire-shape, dual-surface, or CRLF invariants regress"
tech_stack:
  added: []
  patterns:
    - "include_str! at compile time for SKILL.md / reference bodies (no runtime FS access)"
    - "Public Server::builder() path in user-facing examples (validates 80-02 Fix 2 / Codex C3)"
    - "Chained .skill().skill().bootstrap_skill_and_prompt(...) accumulator demo (validates 80-02 Fix 1)"
    - "Content::Resource { uri, text, mime_type } pattern matching at example + test layer (Fix 3)"
    - "Wire-level prompt-handler retrieval via Server::get_prompt(name).handle(...) (Fix 5)"
    - "CRLF + LF mixed-line-endings dual-surface invariant test (Fix 9)"
    - "#![cfg(all(feature = \"skills\", not(target_arch = \"wasm32\")))] gate on integration test (matches 80-01 paired cfg)"
key_files:
  created:
    - "examples/s44_server_skills.rs"
    - "examples/c10_client_skills.rs"
    - "examples/skills/hello-world/SKILL.md"
    - "examples/skills/refunds/SKILL.md"
    - "examples/skills/code-mode/SKILL.md"
    - "examples/skills/code-mode/references/schema.graphql"
    - "examples/skills/code-mode/references/examples.md"
    - "examples/skills/code-mode/references/policies.md"
    - "tests/skills_integration.rs"
  modified:
    - "Cargo.toml"
decisions:
  - "Both examples use pmcp::Server::builder() (returns ServerBuilder per src/server/mod.rs:637) — the public PMCP path validated by 80-02 Fix 2 / Codex C3. No ServerCoreBuilder::new() anywhere in the example main code."
  - "Example filenames are s44_server_skills.rs + c10_client_skills.rs (not s38/c38 as the original prompt requested) because s38_prompt_workflow_progress.rs already exists at Cargo.toml:285."
  - "c10 legacy flow goes through server.get_prompt('start_code_mode').handle(...) — the same path prompts/get executes — NOT a direct skill.as_prompt_text() call (Fix 5 verified at example layer)."
  - "Each c10 read response is matched on Content::Resource { uri, text, mime_type, .. } with explicit assert_eq! on per-resource MIME (Fix 3 verified at example layer)."
  - "Integration test promotes the wire-level dual-surface assertion from optional to mandatory (Test 3.7 per Fix 5 / Codex C6). The test builds a real Server, retrieves the registered prompt handler via get_prompt, invokes .handle, and assert_eq!s vs. the SEP-2640 surface."
  - "Integration test includes a CRLF + LF mixed-line-endings scenario (Test 3.7a per Fix 9 / Gemini) — the dual-surface invariant must hold for both line-ending styles."
  - "SKILL.md + reference bodies match spike 002 lines 391-512 byte-for-byte (verified at write time via Python diff)."
  - "Two proptest harnesses (Tests 3.8 + 3.9) cover construction-level byte equality under arbitrary content AND the Content::Resource wire shape under arbitrary content."
  - "Both [[example]] entries carry required-features = ['skills', 'full'] so they are correctly gated out under default features (verified: cargo build --example s44_server_skills without features fails with 'requires features: skills, full')."
  - "Doc comments in c10 use prose section headings (## Flow A, ## Flow B) instead of a numbered list — the original numbered list tripped clippy's doc_lazy_continuation + doc_overindented_list_items lints (auto-fixed inline per Rule 1)."
metrics:
  duration: "~15 minutes wall clock"
  completed_date: "2026-05-13"
  tasks_completed: 3
  files_created: 9
  files_modified: 1
  insertions: 834
  deletions: 0
  commits: 4    # 3 task + 1 fmt fixup
  new_tests: 10  # 8 unit + 2 proptest harnesses in skills_integration.rs
  cog_impact: 0
---

# Phase 80 Plan 03: SEP-2640 Skills Support — Examples + Integration Test Summary

Shipped the ALWAYS-required ergonomics deliverables that close phase 80: two paired examples (`s44_server_skills` server-side + `c10_client_skills` client-side dual-flow with byte-equality), six SKILL.md/reference content files byte-equal to spike 002, and a 10-test integration suite that locks the wire-shape (Fix 3), wire-level dual-surface (Fix 5), and CRLF resilience (Fix 9) invariants under both unit and proptest coverage.

## What Shipped

| Task | Commit     | Files touched                                                                                                                                                  | Lines |
| ---- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- |
| 1    | `8b07503b` | examples/skills/{hello-world,refunds,code-mode}/SKILL.md + code-mode/references/{schema.graphql,examples.md,policies.md} + examples/s44_server_skills.rs + Cargo.toml | +387  |
| 2    | `ed432911` | examples/c10_client_skills.rs + Cargo.toml                                                                                                                     | +222  |
| —    | `5ba58a94` | examples/c10_client_skills.rs (rustfmt fixup)                                                                                                                  | +4/−1 |
| 3    | `4fa252fd` | tests/skills_integration.rs                                                                                                                                    | +380  |

Total: 4 commits, 10 files, +834/−0.

## Verification

All `<verification>` items from the plan pass:

- `cargo build --features skills,full --example s44_server_skills` — succeeds
- `cargo build --features skills,full --example c10_client_skills` — succeeds
- `cargo run --features skills,full --example s44_server_skills` — runs, exit 0, stdout contains all marker strings (`hello-world`, `acme/billing/refunds`, `code-mode`, `start_code_mode`, `skill://index.json`)
- `cargo run --features skills,full --example c10_client_skills` — runs, exit 0, stdout shows both Flow A (`resources/list` + 4 `resources/read` lines with per-MIME) and Flow B (`prompts/get start_code_mode`), final line: `Both flows produced byte-equal context (2496 bytes).`
- `cargo build --example s44_server_skills` (without skills feature) — fails with `requires the features: skills, full` (correctly gated)
- `cargo build` (default features) — succeeds (zero infection)
- `cargo build --no-default-features` — succeeds (zero infection)
- `cargo test --features skills,full --test skills_integration -- --test-threads=1` — **10 passed** (1 suite, 0.18s)
- `cargo test --test skills_integration` (without skills feature) — emits 0 tests (cfg-gated correctly)
- `cargo clippy --features skills,full --examples --tests -- -D warnings` — zero warnings
- `cargo doc --no-deps -p pmcp --features skills` — renders (pre-existing warnings in `src/server/auth/mod.rs` are unrelated)
- `make quality-gate` — **exits 0** (fmt-check, lint, build, test-all, audit, unused-deps, check-todos, check-unwraps, validate-always all pass)

## Wire-Shape Contract — Fix 3 Locked at Two New Layers

The four `ReadResourceResult` responses are matched on `Content::Resource { uri, text, mime_type, .. }` — never `Content::Text` — at both the example AND integration-test layer:

| Layer                                                  | Location                                                                       |
| ------------------------------------------------------ | ------------------------------------------------------------------------------ |
| Example (developer-facing demo)                        | `examples/c10_client_skills.rs` `extract_resource(...)` + 7× `assert_eq!` calls |
| Integration tests 3.2 (SKILL.md), 3.3 (ref), 3.4 (idx) | `tests/skills_integration.rs` direct unit assertions                            |
| Proptest 3.9                                           | `tests/skills_integration.rs` — every read response under arbitrary content     |

The `application/graphql` MIME on `references/schema.graphql` is the load-bearing case: it proves per-resource MIME survives the wire round-trip (would silently degrade to absent if `Content::Text` were used).

## Dual-Surface Byte-Equality — Fix 5 Promoted to Mandatory

The plan promoted the wire-level test from optional (v1) to mandatory (v2). All five layers are now covered across plans 80-02 and 80-03:

| Layer                              | Test                                                  | Plan      |
| ---------------------------------- | ----------------------------------------------------- | --------- |
| Data construction                  | `test_1_4_as_prompt_text_*` + `test_1_5_*`            | 80-02     |
| Construction property              | `prop_1_19_as_prompt_text_byte_equal_concat`          | 80-02     |
| Handler response                   | `test_1_14_skill_prompt_handler_returns_byte_equal_text` | 80-02  |
| Builder-registered wire path       | `test_2_8_bootstrap_skill_and_prompt_byte_equal_invariant` | 80-02 |
| Integration-test wire path         | `dual_surface_byte_equal_wire_level_via_get_prompt` (Test 3.7) | **80-03 (this plan)** |
| Construction byte-equality         | `dual_surface_byte_equal_construction_level` (Test 3.6) | **80-03** |
| Integration property               | `proptest_byte_equality_under_arbitrary_skill_content` (Test 3.8) | **80-03** |

The c10 example also panics on a dual-surface violation at runtime — making it a load-bearing demo that doubles as smoke detection.

## CRLF Resilience — Fix 9 Locked

`tests/skills_integration.rs::dual_surface_byte_equal_crlf_and_mixed_line_endings` (Test 3.7a) builds two skills — one with `\n` line endings, one with `\r\n` — and asserts byte-equality at BOTH construction AND wire level for each. The dual-surface invariant survives the SKILL.md-on-Windows-vs-Linux scenario. Combined with Plan 80-02's Tests 1.6a (frontmatter CRLF) + 1.6b (BOM), the line-ending contract is locked across the entire pipeline.

## Accumulator Pattern — Fix 1 Demonstrated in s44

The s44 example chains `.skill(Skill::new("hello-world", ...)).skill(Skill::new("refunds", ...).with_path(...)).bootstrap_skill_and_prompt(code_mode_skill, "start_code_mode")` — three calls that, under the v1 design, would have made the first two skills unreachable via `read()`. Under the v2 accumulator + single-finalize design (Plan 80-02), all three skills register correctly and their SKILL.md URIs all appear in `resources/list`. The example serves as the developer-facing proof that the v1 regression cannot recur.

## ALWAYS Requirements Compliance (CLAUDE.md)

| Requirement                         | Status                                                                                                                                            |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Example demonstration**           | Two paired examples (`s44_server_skills` + `c10_client_skills`) both `cargo run` to completion under `--features skills,full`. ALWAYS satisfied.   |
| **Property testing**                | Two proptest harnesses in `tests/skills_integration.rs` (Tests 3.8 + 3.9) at proptest defaults (256 cases each).                                  |
| **Unit testing**                    | Eight unit tests in `tests/skills_integration.rs` (Tests 3.1–3.7a) covering all 4 SEP-2640 endpoints + dual-surface + CRLF.                       |
| **Fuzz testing**                    | Satisfied by the 2 proptest harnesses × 256 cases (512 randomized inputs per run). No separate `cargo fuzz` target — consistent with 80-02's reasoning. |
| **Integration tests**               | The entire `tests/skills_integration.rs` file IS the integration suite.                                                                            |
| **Doctests**                        | No new public API in this plan; the 6 doctests in `src/server/skills.rs` (added in 80-02) remain passing under `cargo test --doc --features skills`. |
| **Cog complexity ≤ 25**             | All example + test functions ≤ 18 cog. `make quality-gate` (which runs PMAT in CI) exits 0.                                                       |
| **Zero SATD**                       | No TODO/FIXME/HACK comments in any new file.                                                                                                       |
| **Comprehensive documentation**     | Both example files have substantive module-level doc comments explaining the SEP-2640 flow, the Fix 3 wire-shape rationale, and Fix 5 wire-level prompt-handler retrieval. The integration test has a module-level doc comment describing the load-bearing tests. |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] clippy `doc_lazy_continuation` + `doc_overindented_list_items` errors on c10 doc comments.**

- Found during: Task 2 clippy verification (`cargo clippy --features skills,full --example c10_client_skills -- -D warnings`).
- Issue: The original c10 doc comment used a numbered list (`1. **SEP-2640 flow** —` / `2. **Legacy prompt flow** —`) with 3-space continuation indent. Rust 1.95's clippy flagged both items: item 1 wanted MORE indent (6 spaces) while item 2 wanted LESS indent (3 spaces) — the rules disagree because of how bold text shifts the wrap target inside a list-item continuation.
- Fix: Replaced the numbered list with two `## Flow A` / `## Flow B` prose section headings. Same content semantics, no list-item lint surface.
- Files modified: `examples/c10_client_skills.rs` (doc-comment-only diff)
- Commit: `ed432911` (Task 2 — folded in).

**2. [Rule 3 — Blocking issue] `cargo fmt --all` reformatted one multi-arg `println!` in c10 after Task 2.**

- Found during: `make quality-gate` fmt-check step after Task 3.
- Issue: rustfmt's default policy for `println!` calls with multiple arguments differs from how the line was originally written.
- Fix: Ran `cargo fmt --all`. 4-line whitespace-only diff committed separately so Task 2's `feat` commit stays semantically clean.
- Files modified: `examples/c10_client_skills.rs`
- Commit: `5ba58a94`.

No structural / architectural / scope deviations. Every load-bearing fix (Fix 1 accumulator, Fix 2 public-builder, Fix 3 wire shape, Fix 5 wire-level dual-surface, Fix 9 CRLF) is exercised at the example AND integration-test layers.

## Authentication Gates

None encountered during execution.

## Known Stubs

None. Every example/test asserts behavior end-to-end; nothing is mocked or placeholder.

## Threat Flags

No new threat surface. The plan's `<threat_model>` (T-80-10 example-panics, T-80-11 spike-content-drift, T-80-12 test-log-disclosure, T-80-13 proptest-DoS, T-80-14 wire-level-fake-pass, T-80-15 MIME-strip, T-80-16 CRLF-break) is fully mitigated by the test design — T-80-14 by Test 3.7, T-80-15 by Tests 3.2/3.3/3.4/3.9, T-80-16 by Test 3.7a.

## Why the Three Atomic Pieces Land Separately

| Piece                                                          | Risk profile                                          | Independence                                                                                                                |
| -------------------------------------------------------------- | ----------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Task 1: SKILL content tree + s44 example + Cargo.toml entry    | Content-only + a thin builder demo                    | s44 can revert without affecting c10 or the integration test.                                                                |
| Task 2: c10 example + Cargo.toml entry                          | Adds wire-shape + dual-surface assertion at runtime   | c10 can revert without affecting s44 or the integration test.                                                                |
| Task 3: Integration test                                        | Adds CI regression coverage                           | Test reverts without affecting either example. The examples + library code would still work; the regression net would shrink. |

Each commit is independently revertable. The fmt fixup (`5ba58a94`) is a separable cosmetic touch-up; reverting it just restores the pre-fmt format and re-introduces the fmt-check fail.

## Success Criteria — Plan-Level

- [x] All 3 tasks executed and committed atomically (`8b07503b`, `ed432911`, `4fa252fd`); 1 fmt fixup (`5ba58a94`).
- [x] `examples/s44_server_skills.rs` exists, uses `pmcp::Server::builder()` (Fix 2 — `grep -c 'pmcp::Server::builder()'` returns 4), demonstrates all three tiers, uses `.bootstrap_skill_and_prompt(...)` for code-mode, runs to completion.
- [x] `examples/c10_client_skills.rs` exists, walks both host flows, asserts byte-equality + `Content::Resource` wire shape (Fix 3 — `grep -c 'Content::Resource'` returns 3), uses `server.get_prompt(...)` for the legacy flow (Fix 5 — `grep -c 'get_prompt('` returns 4), `grep -c 'skill\.as_prompt_text()'` returns 0.
- [x] Six SKILL content files exist under `examples/skills/` matching spike 002 lines 391-512 byte-for-byte (Python diff verification at write time).
- [x] `tests/skills_integration.rs` exists, exercises all four SEP-2640 endpoints (Tests 3.1–3.5), contains the construction-level byte-equal assertion (Test 3.6), the MANDATORY wire-level assertion via `get_prompt` (Test 3.7 per Fix 5), the CRLF resilience test (Test 3.7a per Fix 9), and 2 proptest harnesses (Tests 3.8 + 3.9). All 10 tests pass under `--features skills,full`.
- [x] Two `[[example]]` entries added to `Cargo.toml`, both with `required-features = ["skills", "full"]`.
- [x] Dual-surface invariant asserted at FIVE layers (data, handler, builder, integration-wire, proptest).
- [x] `Content::Resource` wire shape asserted at FOUR layers (handler unit, example, integration unit, integration proptest).
- [x] CRLF resilience verified at TWO layers (frontmatter parser unit tests in 80-02 + dual-surface integration test in 80-03).
- [x] `make quality-gate` exits 0.
- [x] No modifications to STATE.md or ROADMAP.md (orchestrator owns those writes).
- [x] No unrelated dirty files staged or committed (verified at every commit via explicit file paths; the working tree's pre-existing modifications to CLAUDE.md, cargo-pmcp/*, examples/wasm-client/*, etc. are untouched).
- [x] Naming deviation (s44/c10 vs s38/c38) flagged in the plan and respected throughout the example file tree and Cargo.toml entries.

## Self-Check: PASSED

**Created/modified files (all FOUND):**
- examples/s44_server_skills.rs
- examples/c10_client_skills.rs
- examples/skills/hello-world/SKILL.md
- examples/skills/refunds/SKILL.md
- examples/skills/code-mode/SKILL.md
- examples/skills/code-mode/references/schema.graphql
- examples/skills/code-mode/references/examples.md
- examples/skills/code-mode/references/policies.md
- tests/skills_integration.rs
- Cargo.toml (modified)
- .planning/phases/80-sep-2640-skills-support/80-03-SUMMARY.md (this file)

**Commit hashes (all FOUND in `git log --oneline`):**
- 8b07503b — feat(80-03): add s44_server_skills example + spike-002 SKILL.md content tree
- ed432911 — feat(80-03): add c10_client_skills example walking both host flows
- 4fa252fd — test(80-03): add skills integration test covering all 4 SEP-2640 endpoints
- 5ba58a94 — style(80-03): rustfmt fixup for c10_client_skills example
