---
phase: 80-sep-2640-skills-support
plan: 01
subsystem: src/types + src/server
tags: [sep-2640, skills, capabilities, feature-flag, wire-protocol]
dependency_graph:
  requires: []
  provides:
    - "ServerCapabilities.extensions"
    - "skills Cargo feature"
    - "src/server/skills module"
  affects:
    - "Phase 80-02 (DX layer + builder integration)"
    - "Phase 80-03 (examples + integration tests)"
tech_stack:
  added: []
  patterns:
    - "Additive Option<HashMap> field mirroring `experimental`"
    - "Opt-in Cargo feature (NOT in default OR full)"
    - "Paired cfg gate `all(feature = \"skills\", not(target_arch = \"wasm32\"))`"
key_files:
  created:
    - "src/server/skills.rs"
  modified:
    - "src/types/capabilities.rs"
    - "src/server/core_tests.rs"
    - "Cargo.toml"
    - "src/server/mod.rs"
decisions:
  - "ServerCapabilities.extensions is the SEP-2640 §6 wire-correct home (not experimental); ClientCapabilities is NOT modified (server-declared SEP)."
  - "skills feature flag is opt-in: NOT in `default = [\"logging\"]`, NOT in `full`. Per CONTEXT.md Implementation Decision #9."
  - "Module placed at src/server/skills.rs (next to handler-trait-backed siblings like dynamic_resources.rs, simple_resources.rs, simple_prompt.rs)."
  - "Module cfg gate is paired `all(feature = \"skills\", not(target_arch = \"wasm32\"))` from the outset per 80-REVIEWS.md C8 (Codex MEDIUM); avoids cfg rework in 80-02 when ResourceHandler/PromptHandler consumers land."
  - "Module is doc-only in this plan: `//!` comment naming the types/handlers 80-02 will provide. No public exports — zero behavior change."
metrics:
  duration: "~25 minutes wall clock"
  completed_date: "2026-05-13"
  tasks_completed: 3
  files_created: 1
  files_modified: 4
  insertions: 176
  deletions: 0
  commits: 4
  new_tests: 4
  cog_impact: 0
---

# Phase 80 Plan 01: SEP-2640 Skills Support — Protocol Types Foundation Summary

Additive `extensions` field on `ServerCapabilities`, opt-in `skills` Cargo feature, and an empty-but-doc-bearing `src/server/skills.rs` skeleton with a paired-cfg gate from day one — landing the wire-correct foundation that Plan 80-02 builds against.

## What Shipped

| Task | Commit     | Files touched                                            | Lines |
| ---- | ---------- | -------------------------------------------------------- | ----- |
| 1    | `1f607d80` | src/types/capabilities.rs, src/server/core_tests.rs       | +133  |
| 2    | `2ab8060f` | Cargo.toml                                                | +4    |
| 3    | `84313686` | src/server/skills.rs (NEW), src/server/mod.rs              | +40   |
| —    | `175f909b` | src/types/capabilities.rs (rustfmt fixup)                  | −1    |

Total: 4 commits, 5 files, +176/−0.

## Verification

All `<verification>` items from the plan pass:

- `cargo test --lib types::capabilities -- --test-threads=1` — 11 pass (was 7 pre-plan, +4 new: `default_serializes_without_extensions_key`, `extensions_round_trip_byte_equal`, `extensions_and_experimental_coexist`, `extensions_camelcase_serde`).
- `cargo build` (default features) — succeeds.
- `cargo build --no-default-features` — succeeds.
- `cargo build --features full` — succeeds (`grep -E 'full = \[.*skills' Cargo.toml` returns 0 matches; `grep -E 'default = \[.*skills' Cargo.toml` returns 0 matches).
- `cargo build --features skills` — succeeds; the empty skeleton module compiles cleanly.
- `cargo check --target wasm32-unknown-unknown --features skills` — succeeds; the C8 cfg-gate fix is in place (the `skills` module is correctly gated out on wasm).
- `cargo clippy --lib --features skills -- -D warnings` — zero warnings; zero mentions of `skills.rs`.
- `grep '^skills = \[\]' Cargo.toml` — exactly 1 match.
- `make quality-gate` — exits 0 with the `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` banner (fmt-check, lint, build, test-all, audit, unused-deps, check-todos, check-unwraps, validate-always).

## Wire-Shape Contract Locked

The four new tests pin the SEP-2640 §6 wire shape:

| Test                                          | Asserts                                                                  |
| --------------------------------------------- | ------------------------------------------------------------------------ |
| `default_serializes_without_extensions_key`   | `extensions` key absent from default JSON (`skip_serializing_if`).        |
| `extensions_round_trip_byte_equal`            | `{"io.modelcontextprotocol/skills": {}}` round-trips equal.              |
| `extensions_and_experimental_coexist`         | Both top-level sibling keys present; neither nested inside the other.   |
| `extensions_camelcase_serde`                  | Wire name is literally `"extensions"` (camelCase rename is identity).   |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] In-crate struct literal in `core_tests.rs` broke when the new field was added.**
- Found during: Task 1 (TDD GREEN compile)
- Issue: `src/server/core_tests.rs:708-719` enumerated all `ServerCapabilities` fields verbatim in a struct literal. `#[non_exhaustive]` does not apply within the same crate, so adding `extensions` produced `error[E0063]: missing field `extensions`'.
- Fix: Added `extensions: None,` to the literal.
- Files modified: `src/server/core_tests.rs`
- Commit: `1f607d80` (folded into Task 1).

**2. [Rule 3 — Blocking issue] `cargo fmt --all` reformatted three chained accessors in the new tests after Task 1.**
- Found during: `make quality-gate` (fmt-check step).
- Issue: rustfmt's default chain-formatting style differs from how I wrote `round.extensions.as_ref().unwrap()...` in the original Task 1 commit.
- Fix: Ran `cargo fmt --all`; committed the cosmetic diff as a separate `style(80-01)` commit so Task 1 stays semantically clean.
- Files modified: `src/types/capabilities.rs` (test-block whitespace only).
- Commit: `175f909b`.

No other deviations. The plan's scope guard — protocol field, feature flag, module skeleton — held end to end. No Skill / SkillReference / Skills / handler / builder code introduced (those are 80-02). No examples / tests / SUMMARY tests touched (those are 80-03).

## Threat Flags

None. Surface scan of the four modified files turned up no new auth paths, network endpoints, file-access patterns, or trust-boundary changes beyond the plan's existing T-80-01/02/03 disposition (accept/accept/mitigate, all unchanged).

## Authentication Gates

None encountered during execution.

## Known Stubs

The `src/server/skills.rs` module is intentionally empty in this plan (Plan 80-02 will populate it). This is documented contract — see the `//!` doc comment in the file naming the six types Plan 80-02 lands. NOT a stub in the deferred-functionality sense: nothing in this plan or its tests depends on those types existing yet.

## Why the Three Atomic Pieces Land Separately

| Piece                                                       | Wire-shape impact                                              | Code-coupling impact                                                          |
| ----------------------------------------------------------- | -------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| `ServerCapabilities.extensions`                             | Direct (SEP-2640 §6 declaration).                              | Zero — additive field on `#[non_exhaustive]` struct.                          |
| `skills = []` Cargo feature                                 | None.                                                          | Pure metadata; toggles the next piece.                                        |
| `src/server/skills.rs` empty module + cfg-gated declaration | None (module exports nothing).                                 | Zero — `pub mod skills;` is `#[cfg]`-out by default and on wasm.              |

Each commit is independently revertable. Plan 80-02 can `caps.extensions = Some(...)` against commit `1f607d80` and `pub use crate::server::skills::*` against commit `84313686` without depending on anything else in this plan beyond the two foundations.

## Success Criteria — Plan-Level

- [x] `ServerCapabilities.extensions` field with `#[serde(skip_serializing_if = "Option::is_none")]`, sibling to `experimental`, round-trips cleanly.
- [x] `skills = []` in `Cargo.toml [features]`, absent from `default` AND `full`.
- [x] `src/server/skills.rs` exists as an empty-but-doc-bearing module, conditionally registered in `src/server/mod.rs` under `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` (the paired cfg from 80-REVIEWS.md C8).
- [x] 80-02 can `use pmcp::types::ServerCapabilities` and write to `caps.extensions` without further changes here.
- [x] 80-02 can `pub use` and `impl` inside `crate::server::skills::*` without re-stating the wasm cfg gate on every item.
- [x] Zero breaking changes to any public API; zero behavior change for any caller that does not opt into `--features skills`; `cargo check` succeeds on `wasm32-unknown-unknown` with `--features skills`.

## Self-Check: PASSED

**Created/modified files (all FOUND):**
- src/server/skills.rs (NEW)
- src/types/capabilities.rs
- Cargo.toml
- src/server/mod.rs
- src/server/core_tests.rs
- .planning/phases/80-sep-2640-skills-support/80-01-SUMMARY.md (this file)
- .planning/phases/80-sep-2640-skills-support/80-01-PLAN.md
- .planning/phases/80-sep-2640-skills-support/80-CONTEXT.md
- .planning/phases/80-sep-2640-skills-support/80-REVIEWS.md
- .planning/phases/80-sep-2640-skills-support/80-02-PLAN.md (staged for history)
- .planning/phases/80-sep-2640-skills-support/80-03-PLAN.md (staged for history)

**Commit hashes (all FOUND in `git log --oneline --all`):**
- 1f607d80 — feat(80-01): add `extensions` field on `ServerCapabilities` (SEP-2640 §6)
- 2ab8060f — feat(80-01): add opt-in `skills` Cargo feature flag
- 84313686 — feat(80-01): add empty `src/server/skills.rs` skeleton + cfg-gated registration
- 175f909b — style(80-01): rustfmt fixup for `extensions` round-trip tests

