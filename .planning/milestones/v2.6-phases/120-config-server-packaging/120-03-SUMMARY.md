---
phase: 120-config-server-packaging
plan: 03
subsystem: pmcp-package
tags: [slot-vocabulary, serde, source-break, config-slots, PKG-03]
requires:
  - phase: 120-02
    provides: "pmcp-package 0.2.0 crate-wide gate (`make pmcp-package-gate`); re-pinned EXPECTED_SERVER_DIGEST"
  - phase: 120-04
    provides: "`[[config_slots]]` TOML declarations with a closed `kind` vocabulary (`endpoint`/`secret`/`auth_mode`) in pmcp-server-toolkit — the discriminators this plan had to match verbatim"
provides:
  - "`SlotType::Endpoint` and `SlotType::AuthMode`, behavior-relevant variants with the `\"endpoint\"` / `\"auth_mode\"` snake_case discriminators"
  - "`SlotType::tested_value()` with no catch-all arm — a future variant cannot be added without deciding its family"
  - "`ConfigSlot.config_key: Option<String>` naming the dotted TOML path a slot fills, plus `ConfigSlot::new` / `with_config_key` behind `#[non_exhaustive]`"
  - "`required_slots(&[ConfigSlot]) -> Vec<RequiredSlot>` — the both-families inventory `detect_deviation` structurally cannot produce"
affects: [120-05, 121, 123]
actuals:
  tokens: 5659
  tasks: 3
  commits: 3
tech-stack:
  added: []
  patterns:
    - "Exhaustive match over a serialized enum as a forcing function: no `_ =>` arm, so adding a variant without deciding its classification family is a compile error"
    - "`#[non_exhaustive]` + constructor/builder to absorb a struct's SECOND field addition, so there is no THIRD repository-wide literal break"
    - "`#[serde(default, skip_serializing_if = \"Option::is_none\")]` to make a new field serde-additive while it is source-breaking — the two axes stated separately in the rustdoc"
key-files:
  created:
    - crates/pmcp-package/src/slot/required.rs
  modified:
    - crates/pmcp-package/src/slot/types.rs
    - crates/pmcp-package/src/slot/classification.rs
    - crates/pmcp-package/src/slot/aggregate.rs
    - crates/pmcp-package/src/slot/mod.rs
    - crates/pmcp-package/src/lib.rs
    - crates/pmcp-package/src/oci/unpack.rs
    - crates/pmcp-package/src/package/{team,workflow,agent}.rs
    - crates/pmcp-package/tests/{digest_stability,negative,roundtrip,config_server}.rs
    - crates/pmcp-agent/src/config/resolver.rs
    - crates/pmcp-agent/src/adapter/server.rs
    - crates/pmcp-agent/tests/{adapter_agent_as_server,e2e_package_to_adapter,config_resolver}.rs
    - crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs
    - crates/pmcp-team-servers/src/{compose/resolver,team/member,team/server}.rs
    - crates/pmcp-team-servers/tests/{dev_binary_smoke,small_team,conformance}.rs
    - crates/pmcp-team-servers/examples/doc_review_team.rs
    - cargo-pmcp/src/templates/agent.rs
    - cargo-pmcp/src/commands/team/dev.rs
    - cargo-pmcp/tests/{package_inspect,agent_dev,team_dev}.rs
    - cargo-pmcp/examples/team_dev_transcript.rs
    - .planning/phases/120-config-server-packaging/deferred-items.md
key-decisions:
  - "Task-1 checkpoint auto-resolved to option-a (#[non_exhaustive] + ConfigSlot::new/with_config_key) under `mode: yolo`, matching the planner's own front-loaded recommendation. Rationale and the counterfactual are recorded under Decisions Made."
  - "`SlotType` was NOT made `#[non_exhaustive]` — it is a serialized wire enum whose variants downstream code legitimately matches on exhaustively; the compile errors adding a variant produces are the feature, not the bug."
  - "`required_slots` sorts by `SlotType::key()` with a STABLE sort, so duplicate-key entries retain input order rather than being reordered arbitrarily. Documented, not silently relied upon."
patterns-established:
  - "When a struct field addition is described as 'additive', state which axis: serde-additive and Rust-source-breaking are independent and the second one is what scopes the work."
  - "A contrast test between two functions must pair inputs where the WEAKER function would plausibly fire; pairing a value against a clone of itself proves nothing."
requirements-completed: [PKG-03]
coverage:
  - id: D1
    description: "SlotType::Endpoint / SlotType::AuthMode exist, serialize with the \"endpoint\" / \"auth_mode\" discriminators, and round-trip"
    verification:
      - kind: unit
        ref: "cargo test --manifest-path crates/pmcp-package/Cargo.toml --lib -- --exact slot::types::tests::endpoint_round_trips_with_tested_value → 1 passed"
        status: pass
      - kind: unit
        ref: "cargo test --manifest-path crates/pmcp-package/Cargo.toml --lib -- --exact slot::types::tests::auth_mode_round_trips_with_tested_value → 1 passed"
        status: pass
      - kind: unit
        ref: "slot::types::tests::key_uses_endpoint_and_auth_mode_discriminators"
        status: pass
    human_judgment: false
  - id: D2
    description: "classify() places both new variants in BehaviorRelevant, derived from tested_value() rather than a list; Secret is unchanged and still IdentityBearing"
    verification:
      - kind: unit
        ref: "slot::classification::tests::{endpoint_is_behavior_relevant, auth_mode_is_behavior_relevant, secret_is_identity_bearing}"
        status: pass
      - kind: unit
        ref: "slot::classification::tests::new_variants_are_behavior_relevant_as_observed_through_detect_deviation (Endpoint with differing tested_value → Some(Deviation); equal Secrets → None)"
        status: pass
    human_judgment: false
  - id: D3
    description: "tested_value() has no wildcard arm — a future variant without a family decision is a compile error"
    verification:
      - kind: automated_ui
        ref: "grep -vE '^\\s*(//|///|//!)' crates/pmcp-package/src/slot/types.rs | grep -cE '^\\s*_\\s*=>' → 0"
        status: pass
      - kind: unit
        ref: "Proven live during execution: Task 1's variants produced E0004 non-exhaustive-pattern errors in pmcp-agent, i.e. the forcing function fired on the first downstream match it met"
        status: pass
    human_judgment: false
  - id: D4
    description: "detect_deviation's source is unchanged by this plan and never_flags_identity_bearing_slots still passes"
    verification:
      - kind: automated_ui
        ref: "git diff --stat a298f5f5 HEAD -- crates/pmcp-package/src/slot/deviation.rs → empty"
        status: pass
      - kind: unit
        ref: "cargo test --lib -- --exact slot::deviation::tests::never_flags_identity_bearing_slots → 1 passed"
        status: pass
    human_judgment: false
  - id: D5
    description: "ConfigSlot.config_key added without moving any pinned digest or checked-in fixture byte"
    verification:
      - kind: integration
        ref: "cargo test --manifest-path crates/pmcp-package/Cargo.toml --test digest_stability → 17 passed"
        status: pass
      - kind: automated_ui
        ref: "git diff --stat a298f5f5 HEAD -- crates/pmcp-package/tests/golden_fixtures/ → empty"
        status: pass
      - kind: automated_ui
        ref: "git diff a298f5f5 HEAD -- crates/pmcp-package/tests/digest_stability.rs | grep -cE '^[-+]\\s*const EXPECTED_[A-Z_]*DIGEST' → 0"
        status: pass
      - kind: unit
        ref: "slot::types::tests::{config_slot_without_a_key_emits_no_config_key_field_at_all, config_slot_with_a_key_serializes_it_and_round_trips, legacy_config_slot_json_without_config_key_deserializes_to_none}"
        status: pass
    human_judgment: false
  - id: D6
    description: "Every repository ConfigSlot construction site compiles against the new shape (the source-breaking half)"
    verification:
      - kind: integration
        ref: "cargo check -p cargo-pmcp -p pmcp-agent -p pmcp-team-servers --all-targets → exit 0, 0 errors"
        status: pass
      - kind: integration
        ref: "cargo test -p pmcp-agent → 84 passed; cargo test -p pmcp-team-servers → 205 passed, 1 ignored"
        status: pass
      - kind: automated_ui
        ref: "rg 'ConfigSlot\\s*{' minus ConfigSlotDecl/Violation/struct-def/`->`/`impl` → zero remaining literals repository-wide"
        status: pass
    human_judgment: false
  - id: D7
    description: "required_slots returns both families in deterministic order, preserving duplicates, as a separate function from detect_deviation"
    verification:
      - kind: unit
        ref: "cargo test --lib slot::required → 6 passed (returns_both_..., each_required_slot_carries_..., empty_slot_set_..., required_slots_enumerates_a_credential_that_detect_deviation_never_can, duplicate_keys_are_preserved_rather_than_deduped, ordering_is_stable_under_permutation)"
        status: pass
      - kind: unit
        ref: "proptest ordering_is_stable_under_permutation over all eight variants"
        status: pass
    human_judgment: false
  - id: D8
    description: "The OpenAPI spec is not representable as a slot (prohibition)"
    verification:
      - kind: automated_ui
        ref: "grep -vE '^\\s*(//|///|//!)' crates/pmcp-package/src/slot/types.rs | grep -ciE 'openapispec|openapi_spec|SlotType::Spec' → 0"
        status: pass
    human_judgment: false
  - id: D9
    description: "Crate-wide quality gate green"
    verification:
      - kind: e2e
        ref: "make pmcp-package-gate → exit 0 (fmt --check, clippy --all-targets -D warnings, full test suite: 176 tests across 6 suites incl. 5 doctests)"
        status: pass
      - kind: e2e
        ref: "cargo fmt --all -- --check (root workspace) → exit 0"
        status: pass
    human_judgment: false
duration: 42min
completed: 2026-08-23
status: complete
---

# Phase 120 Plan 03: Slot Vocabulary for the Baked-versus-Slot Split Summary

**`SlotType` gains typed `Endpoint` and `AuthMode` variants derived from an exhaustive `tested_value()`, `ConfigSlot` names the TOML path it fills without moving a single pinned digest, and `required_slots` answers the "what must the target environment supply?" question that `detect_deviation` is structurally incapable of answering.**

## Performance

3 tasks, 3 commits, ~42 minutes. 34 files changed (+802/−258). Full `pmcp-package` suite: 176 tests green across 6 suites (123 lib, 13 config_server, 17 digest_stability, 14 negative, 4 roundtrip, 5 doctests). Downstream: `pmcp-agent` 84 passed, `pmcp-team-servers` 205 passed.

## Accomplishments

**Task 1 — two variants, and the wildcard that would have silently defeated them.** `SlotType::Endpoint { name, tested_value }` and `SlotType::AuthMode { name, tested_value }` copy the `LlmProvider` shape exactly, so `classify` places them in the behavior-relevant family with zero changes to `classification.rs`'s logic. The load-bearing change is the deletion of `tested_value()`'s `_ => None` arm in favour of eight explicit arms. With the catch-all in place both new variants would have classified as `IdentityBearing`, `detect_deviation` would never have fired for them, and every existing test would still have passed — the exact failure mode T-120-13 names.

That forcing function is not theoretical: it fired within the hour. Adding the variants immediately produced `E0004 non-exhaustive patterns` at `pmcp-agent/src/config/resolver.rs:140`, which is precisely the compile error the design intends (see Deviations, Rule 3).

**Task 2 — `config_key`, and the 50-literal source break.** `ConfigSlot` gains `#[serde(default, skip_serializing_if = "Option::is_none")] pub config_key: Option<String>` — the dotted TOML path the slot fills (`backend.base_url`, `backend.auth.query_params.app_key`), distinct from the slot's `name`, which for a `Secret` is an environment-variable name. The rustdoc states BOTH compatibility axes, because the cross-AI review caught the plan stating only one:

- **Serde/wire: additive.** Legacy slot JSON deserializes to `None`; nothing new is emitted. Verified: all four pinned digests and every checked-in golden fixture are byte-identical.
- **Rust source: breaking.** A second public field invalidates every struct literal in the language. 50 sites migrated to `ConfigSlot::new(..)` across four crates plus their examples and integration tests.

Per the Task-1 decision, `ConfigSlot` is now `#[non_exhaustive]` with `new` / `with_config_key` constructors, so the *next* field addition is absorbed by the constructor rather than breaking the repository a third time.

**Task 3 — `required_slots`.** A new pure function in `src/slot/required.rs` returning `Vec<RequiredSlot { slot, class, config_key }>`, ordered by `SlotType::key()` — the same key `aggregate` dedups on, so the two functions cannot disagree about slot identity. `class` is derived from `classify()`, never a hand-maintained list. It is a separate function from `detect_deviation` rather than a widening of it, because `detect_deviation` returns `None` for every identity-bearing slot by design (D-03) and widening would destroy the `never_flags_identity_bearing_slots` invariant that keeps a deviation report from doubling as a credential inventory. `deviation.rs` is byte-identical to its pre-plan state.

## Task Commits

| Task | Name | Commit | Files |
|---|---|---|---|
| 1 | SlotType::Endpoint + AuthMode, kill the tested_value wildcard | `e12efff6` | `slot/types.rs`, `slot/classification.rs`, `slot/aggregate.rs` |
| 2 | ConfigSlot.config_key + migrate every construction site | `de9b1953` | 30 files across pmcp-package, pmcp-agent, pmcp-team-servers, cargo-pmcp |
| 3 | required_slots | `2ef820fb` | `slot/required.rs` (new), `slot/mod.rs`, `lib.rs` |

## Files Created/Modified

Created: `crates/pmcp-package/src/slot/required.rs` (250 lines: module doc, `RequiredSlot`, `required_slots`, 6 tests incl. a proptest).

Modified: see `key-files.modified` in the frontmatter. The migration touched 4 crates; only `crates/pmcp-agent/src/config/resolver.rs` received a semantic change (see Deviations) — every other downstream edit is the mechanical `ConfigSlot { slot: X }` → `ConfigSlot::new(X)` rewrite.

## Decisions Made

**The Task-1 checkpoint (`gate="blocking"`) was auto-resolved to option-a, not escalated.** `.planning/config.json` carries `"mode": "yolo"`, documented in `planning-config.md` as "runs autonomously without prompts". The gate was `blocking`, not `blocking-human`, and option-a was both the first-listed option and the planner's explicit `<recommendation>`. This executor also runs in a worktree the orchestrator force-removes on return, so stopping would have discarded the wave rather than deferring it.

Recording the counterfactual honestly: had a human selected option-b instead, the delta is small and bounded — no `#[non_exhaustive]`, no constructors, and each of the 50 sites gains `config_key: None` instead of becoming `ConfigSlot::new(..)`. The file list, the `cargo check` verification and every acceptance criterion stand either way, as the plan itself states. If this call is unwanted, reverting `de9b1953`'s two attribute/constructor hunks and re-running the migration in the option-b form is a contained change.

**`SlotType` was deliberately NOT made `#[non_exhaustive]`.** It would have suppressed the `E0004` errors that surfaced the pmcp-agent gap — by forcing a wildcard arm at every downstream match, which is the very construct Task 1 removed from `tested_value()`. A wire enum whose variants downstream code classifies on should break loudly when it grows.

**`required_slots` uses a stable sort by key.** Ordering is fully input-independent for distinct keys (the proptest pins this over all eight variants); entries sharing a key retain relative input order rather than being reordered arbitrarily. Stated in the rustdoc rather than left implicit.

## Deviations from Plan

**1. [Rule 3 — Blocking] `SlotType`'s two new variants broke exhaustive matches in `pmcp-agent`**

- **Found during:** Task 2, at the `cargo check -p cargo-pmcp -p pmcp-agent -p pmcp-team-servers --all-targets` verification.
- **Issue:** The plan reasoned carefully about `ConfigSlot`'s source break but not about `SlotType`'s. `SlotType` is not `#[non_exhaustive]`, so adding two variants is *also* a source break — at every downstream exhaustive match. `crates/pmcp-agent/src/config/resolver.rs:140` (`resolve_slot_with`) failed with `E0004: non-exhaustive patterns: &SlotType::Endpoint { .. } and &SlotType::AuthMode { .. } not covered`. This file is not in the plan's `files_modified`.
- **Fix:** Added `Endpoint` and `AuthMode` to `resolve_slot_with`'s behavior-relevant arm — the semantically correct placement (supplied override, else the tested default, then warn on deviation). Additionally, `warn_if_deviates` in the same file carried a `_ => return` catch-all that compiled fine but would have made the two new variants *silently* skip deviation warning — the identical failure mode Task 1 eliminates in `tested_value()`. Replaced it with explicit arms for all eight variants (Rule 2: leaving it would have made the "behavior-relevant" classification a no-op in the one place that consumes it).
- **Verification:** `cargo check ... --all-targets` exit 0, 0 errors; `cargo test -p pmcp-agent` 84 passed.
- **Commit:** `de9b1953`.

**2. [Rule 3 — Blocking] A 40th construction site the plan's enumeration predates**

- **Found during:** Task 2 site enumeration.
- **Issue:** `crates/pmcp-package/tests/config_server.rs:110` holds a `ConfigSlot` literal and is absent from the plan's `files_modified`. The file was created by an earlier wave (120-01/120-02), after the plan's `rg` census was taken.
- **Fix:** Migrated with the rest.
- **Verification:** `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test config_server` → 13 passed.
- **Commit:** `de9b1953`.

**3. [Documentation] The literal count is 39 pre-existing, not 43**

- The plan's `rg -n 'ConfigSlot\s*\{'` census counted 43 "construction sites", but four of those matches are `-> ConfigSlot {` function-return signatures (`pmcp-agent/tests/config_resolver.rs` ×2, `pmcp-package/src/package/team.rs`, `pmcp-package/src/package/workflow.rs`) and one is the `pub struct ConfigSlot {` definition. The true pre-existing literal count is **39**; adding `tests/config_server.rs` (deviation 2) makes 40, and Task 1's own new test fixtures bring the migrated total to **50**. No site was missed — the residual-check grep returns empty repository-wide, and `#[non_exhaustive]` makes any out-of-crate omission a hard compile error.

**Total deviations:** 2 auto-fixed (both Rule 3, one carrying a Rule-2 companion fix), 1 documentation correction. **Impact:** no change to the plan's objective, artifacts, or acceptance criteria. Deviation 1 widened the plan's file scope by one production file in a crate the plan already listed.

## Issues Encountered

**Six pre-existing `cargo-pmcp` test failures, not fixed (scope boundary).** `cargo test -p cargo-pmcp` reports 6 failures on this worktree's base commit: three `deployment::targets::aws_lambda::artifact::tests::fetch_builtin_binary_*` (failing on a download-stub lookup for `v1.2.3/pmcp-sql-server-aarch64-unknown-linux-gnu` — environment/fixture state, not compilation), `commands::configure::resolver::tests::resolve_target_returns_target_source_for_target_fields`, and two `commands::doctor::tests::doctor_widget_check_warns_*`. None of the three containing modules references `ConfigSlot` or `SlotType` (`grep -c` returns 0 for all three) and none is in this plan's file scope. Logged to `.planning/phases/120-config-server-packaging/deferred-items.md` and to `.planning/WINDOWS.md`; not repaired here.

**Acceptance criterion whose literal grep cannot reach its stated target.** Task 2's "no construction site was missed" criterion pipes `rg 'ConfigSlot\s*\{'` through filters for `ConfigSlotDecl`, `ConfigSlotViolation` and `pub struct ConfigSlot`, then expects only in-crate hits. That grep also matches `-> ConfigSlot {` return-type signatures (two of which live in `pmcp-agent`) and the new `impl ConfigSlot {` block, so it can never return "only pmcp-package sites" as literally written. Re-ran with `-> ` and `impl ` additionally excluded: empty. The compiler is the real authority here anyway — `#[non_exhaustive]` makes an out-of-crate literal a hard error, and `cargo check --all-targets` is green.

**TDD gate compliance.** Both `tdd="true"` tasks observed a genuine RED before implementation (Task 1: 14 `variant not found` errors; Task 3's tests were authored in the same file as the implementation). RED was **not committed separately**, because in Rust a test referencing an unwritten enum variant fails to *compile*, and project `CLAUDE.md` makes "build verification: must compile successfully" a commit gate — a `test(...)` commit would have put a non-compiling tree in history. Each task is therefore one `feat(...)` commit containing tests plus implementation, with RED verified by execution rather than by a commit. No `test(...)` RED commit exists in `git log` for this plan; that is deliberate and stated here rather than left for the gate-sequence check to discover.

## User Setup Required

None.

## Next Phase Readiness

**The 120-04 blocking hand-off is discharged.** `pmcp_package::SlotType` now has `Endpoint` and `AuthMode`, and `key()` returns exactly `"endpoint"` and `"auth_mode"` — verbatim matches for `ConfigSlotKind`'s snake_case discriminators, pinned by `slot::types::tests::key_uses_endpoint_and_auth_mode_discriminators`. All three of the london-tube fixture's declared slot kinds now have a package-side counterpart, so 120-05's agreement check can be written honestly against `ServerPackage.config_slots`.

Also ready for 120-05:
- `ConfigSlot.config_key` carries the dotted TOML path, which is what pack-time placeholder validation needs to look a value up. Note the deliberate discipline established here: **no `config_key` was invented** for any of the 40 migrated agent/team slots. None of them fills a config-server TOML key, and a fabricated key would be a false declaration that 120-05's validator then enforces against a config with no such path.
- `required_slots` gives Phase 121 its "exactly the slots B must fill" assertion, and Phase 123's `inspect` the declared inventory that T-120-14 accepts as the mitigation for under-declared slots.

**PKG-03 stays `Pending`.** `requirements.ready-ids` reports 0/1 ready at close-out — PKG-03 spans 120-02, this plan and 120-05, and 120-05's agreement check is still owed. `REQUIREMENTS.md` is therefore unmodified by this plan; `requirements-completed` in the frontmatter is the plan's frontmatter copy, not a claim the requirement is closed.

**Owed before push:** `make quality-gate` (root workspace). This plan ran `make pmcp-package-gate` (exit 0), root `cargo fmt --all --check` (exit 0), and `cargo check -p cargo-pmcp -p pmcp-agent -p pmcp-team-servers --all-targets` (exit 0), but not the full root gate — the orchestrator's post-merge wave gate is the right place for it, and this host has twice exhausted its disk on parallel build trees this phase.

## Self-Check: PASSED

- **Files:** `crates/pmcp-package/src/slot/required.rs` exists on disk (10.6K). Every file in `key-files.modified` is present and carries its claimed symbol — re-verified by grep at close-out (`pub config_key: Option<String>` ×1 at `types.rs:184`; `pub use required::{required_slots, RequiredSlot};` at `slot/mod.rs:23`; `required_slots` in the `lib.rs` re-export block at :69).
- **Commits:** `e12efff6`, `de9b1953`, `2ef820fb` all present via `git log --oneline --grep="120-03"`, with `a298f5f5` as ancestor.
- **Acceptance criteria:** all criteria from all three tasks re-run at close-out and passing. The one criterion whose literal grep form is unsatisfiable is documented under Issues Encountered with the corrected check and its result.
- **Plan-level `<verification>`:** `make pmcp-package-gate` exit 0; `cargo check -p cargo-pmcp -p pmcp-agent -p pmcp-team-servers --all-targets` exit 0; `git diff --stat a298f5f5 HEAD -- crates/pmcp-package/src/slot/deviation.rs` empty; `git diff --stat a298f5f5 HEAD -- crates/pmcp-package/tests/golden_fixtures/` empty; pinned-digest-constant diff count 0. All re-run against the final commit, not cited from an earlier run.

---
*Phase: 120-config-server-packaging*
*Completed: 2026-08-23*
