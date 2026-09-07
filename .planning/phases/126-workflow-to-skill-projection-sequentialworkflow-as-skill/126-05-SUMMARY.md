---
phase: 126-workflow-to-skill-projection-sequentialworkflow-as-skill
plan: 05
subsystem: api
tags: [skills, sep-2640, workflow, prompt-handler, builder, projection, d-04a]
status: complete

# Dependency graph
requires:
  - phase: 126-01
    provides: "`SequentialWorkflow::as_skill()`, the `skills/` module split, and the ANSWERED gate B (`add-builder-path`) this plan's Task 4 implements"
  - phase: 126-02
    provides: "the full D-11 render — the body whose bytes become prompt message [0]"
  - phase: 126-04
    provides: "`SkillProjection` / `ProjectionOutput`; untouched here, but the same `project()` seam backs the cached prepend"
provides:
  - "`WorkflowPromptHandler::with_projected_skill_prepend(bool)` — the `#[must_use]`, default-off D-04a opt-in that renders the projected body ONCE and caches it as a `String`"
  - "`WorkflowPromptHandler::projected_prepend()` — one crate-private producer, called by BOTH handler kinds, so they cannot drift"
  - "`ServerCoreBuilder::with_workflow_skill_prepend(bool)` and `ServerBuilder::with_workflow_skill_prepend(bool)` — GATE B's builder path, making the anti-drift claim hold per SERVER and not merely per workflow value"
  - "Ten transcript/reachability tests in `tests/skills_integration.rs` (`make test-skills` selector 3: 15 -> 25)"
  - "A measured byte-identity proof that flag-off transcripts did not move"
affects: [126-06, 126-07, workflow-prompts, changelog, examples]

# Actuals (#2632) — estimateTokens scale (chars/4 over the realized diff).
actuals:
  tokens: 5552
  tasks: 4
  commits: 4

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Render-once-and-cache at set time rather than compute-per-request, when the cached value must be the SAME string another surface already published"
    - "Skills-off `#[cfg(not(feature = \"...\"))]` twin of a helper so the calling code needs no `#[cfg]` branch and the feature-off path is literally the same code"
    - "Anti-vacuity by defect injection: red the test with the exact defect it claims to catch, then revert"

key-files:
  created: []
  modified:
    - src/server/workflow/prompt_handler.rs
    - src/server/workflow/task_prompt_handler.rs
    - src/server/builder.rs
    - src/server/mod.rs
    - tests/skills_integration.rs

key-decisions:
  - "GATE B implemented as answered (`add-builder-path`): one `#[cfg]`'d default-false field, one `#[must_use]` chainable setter, and one chained call at handler construction on BOTH `ServerCoreBuilder` and `ServerBuilder`. Two new public methods on published 2.x builder types — one-way."
  - "In `ServerCoreBuilder` the chained `.with_projected_skill_prepend(..)` sits BEFORE the `TaskWorkflowPromptHandler` wrap, so a task-enabled workflow inherits the setting through its `inner` handler and there is no second call site to keep in sync."
  - "The builder setting is read at REGISTRATION time, so it applies to workflows registered AFTER the call. Stated in both rustdocs and shown in both doctests; an ordering-sensitive builder method with no doc is a trap."
  - "`create_assistant_plan()?` is CALLED unconditionally in both handlers and only its MESSAGE is suppressed (REVIEWS fable (a)). Guarding the call would have silently deleted the unregistered-tool validation whenever the flag was on."
  - "The prepend field holds an already-rendered `String`, not a `bool` (REVIEWS fable (b)): a per-request render would re-fire the D-15 slug `tracing::warn!` on every `prompts/get` and re-derive message [0] independently of the digested snapshot."
  - "`ServerCoreBuilder`'s new field uses `#[cfg(all(feature = \"skills\", not(target_arch = \"wasm32\")))]` rather than the plan's bare `#[cfg(feature = \"skills\")]` — that struct compiles on wasm32 while its only reader (`prompt_workflow`) does not, and `all(..)` is this file's existing convention for every skills member. `ServerBuilder` uses the bare gate as the plan specified, because the whole struct is already `not(wasm32)`."

patterns-established:
  - "One crate-private producer for a value two independent code paths must agree on — `projected_prepend()` — instead of a state accessor plus duplicated logic. `prepend.is_some()` IS the flag, read without exposing the field."
  - "The `_meta` assertion as branch proof: `TaskWorkflowPromptHandler`'s delegating branch carries no `_meta`, so asserting the stub's minted `task_id` is what makes the two-handler test non-vacuous."
  - "Empirical byte-identity: dump the artifact at the pre-plan commit and at HEAD via a throwaway test and `shasum` them, rather than pinning literals and calling it unchanged."

requirements-completed: [SC-5, SC-2]

coverage:
  - id: D1
    description: "D-04a: with the flag ON, prompt message [0] is byte-equal to the projected skill body, `create_user_intent` is kept at [1], and `create_assistant_plan`'s message is suppressed."
    requirement: "SC-5"
    verification:
      - kind: integration
        ref: "tests/skills_integration.rs#flag_on_message_zero_is_the_skill_body"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#flag_on_keeps_user_intent_at_index_one"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#flag_on_suppresses_assistant_plan"
        status: pass
    human_judgment: false
  - id: D2
    description: "D-04 unchanged-execution: with the flag OFF — the default — the transcript is byte-identical to the pre-plan tree's, message for message and role for role."
    requirement: "SC-5"
    verification:
      - kind: integration
        ref: "tests/skills_integration.rs#flag_off_transcript_is_unchanged (role AND text for all 6 messages, plus explicit-false == default)"
        status: pass
      - kind: other
        ref: "shasum -a 256 of the serialized flag-off GetPromptResult at aae4d5c5 vs HEAD: 0479feef0481ebef90f9f347986309a86613afcde3500de24205bb84bbc18e16 on both"
        status: pass
      - kind: unit
        ref: "RUSTFLAGS=\"\" cargo test -p pmcp --features \"full\" --lib -- --test-threads=1 (2032 passed)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Pitfall 6: `TaskWorkflowPromptHandler` produces the SAME message [0] on BOTH branches, including the independent one that rebuilds its own message list."
    requirement: "SC-5"
    verification:
      - kind: integration
        ref: "tests/skills_integration.rs#both_handlers_produce_the_same_message_zero (stub TaskRouter + `_meta` task_id assertion proving the independent branch ran)"
        status: pass
      - kind: other
        ref: "defect injection: dropping the task handler's `messages.extend(prepend)` reddens this test with the projected body vs user-intent mismatch; reverted"
        status: pass
    human_judgment: false
  - id: D4
    description: "D-05: the prepended text and the served skill bytes are the SAME string, proven across both surfaces in one test."
    requirement: "SC-2"
    verification:
      - kind: integration
        ref: "tests/skills_integration.rs#prepended_body_equals_served_skill_bytes (real `ResourceHandler` from `Skills::into_handler()`)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Enabling the flag does not remove the unregistered-tool validation: both flag states fail identically."
    requirement: "SC-5"
    verification:
      - kind: integration
        ref: "tests/skills_integration.rs#flag_on_still_rejects_an_unregistered_tool"
        status: pass
      - kind: other
        ref: "defect injection: guarding `create_assistant_plan()`'s CALL rather than its push reddens this test; reverted"
        status: pass
    human_judgment: false
  - id: D6
    description: "GATE B reachability: a server registering a workflow through `prompt_workflow` on either builder can enable the prepend with one chainable call, and the default stays off."
    requirement: "SC-5"
    verification:
      - kind: integration
        ref: "tests/skills_integration.rs#server_core_builder_prompt_workflow_reaches_the_prepend"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#server_core_builder_prompt_workflow_defaults_to_no_prepend"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#server_builder_prompt_workflow_reaches_the_prepend (positive + negative)"
        status: pass
    human_judgment: false
  - id: D7
    description: "The quality gate can actually reach the D-04a tests: they live in `tests/skills_integration.rs`, which `make test-skills` selector 3 runs."
    requirement: "SC-5"
    verification:
      - kind: integration
        ref: "RUSTFLAGS=\"\" make test-skills — selector 3 reports 25 passed (was 15), all four selectors green, exit 0"
        status: pass
    human_judgment: false

duration: 54 min
completed: 2026-09-04
---

# Phase 126 Plan 05: Workflow Prompt Prepend (D-04a) Summary

An opt-in, default-off, render-once flag makes the projected skill body prompt message `[0]` on both workflow handler kinds — and, under GATE B's `add-builder-path` answer, reaches that opt-in from `prompt_workflow` on both builders, so the "one string, one digest" claim holds per server rather than per workflow value.

## Performance

- **Duration:** 54 min (2026-09-04 17:42Z -> 18:36Z)
- **Tasks:** 4 of 4
- **Commits:** 4 (one per task)
- **Files modified:** 5 (0 created)
- **Net:** +916 / -3 lines

## Accomplishments

1. **The opt-in, rendered exactly once.** `WorkflowPromptHandler::with_projected_skill_prepend(bool)` — `#[must_use]`, `#[cfg(feature = "skills")]`, chainable, default off. The body is rendered in the setter and stored as a `String`; `prompts/get` never calls `as_skill()`. `grep -v '^\s*//' src/server/workflow/prompt_handler.rs | grep -c 'as_skill'` returns exactly **1**, inside the setter.

2. **One shared producer.** `pub(crate) fn projected_prepend(&self) -> Option<PromptMessage>` clones the stored string into a user-role message and performs no rendering. A `#[cfg(not(feature = "skills"))]` always-`None` twin means neither `handle` nor the task handler needs a `#[cfg]` around its statement sequence — the flag-off path is literally the same code in both feature configurations.

3. **Wired at the load-bearing position.** The prepend is inserted before the `create_user_intent` push, so the projected body is message `[0]` on all five later early-return/`break` exits that each build a `GetPromptResult` from the accumulated `messages`.

4. **Validation preserved, message suppressed.** `create_assistant_plan()?` runs unconditionally in both handlers — it is where `Error::Internal("Tool '{}' not found in registry")` is raised — and only the push of its message is guarded. Both call sites carry a `// Why:` comment so the next reader does not "optimise away" an apparently unused call.

5. **Both `TaskWorkflowPromptHandler` branches.** The delegating branch inherits the prepend through `self.inner.handle(..)`; the independent branch now reproduces the same five-step header sequence through the same producer. `grep -v '^\s*//' src/server/workflow/task_prompt_handler.rs | grep -c 'as_skill'` returns **0** — no second render, no exposed field.

6. **GATE B's builder path.** `ServerCoreBuilder` and `ServerBuilder` each gained one default-`false` field and one `#[must_use]` `with_workflow_skill_prepend(bool)`. In `ServerCoreBuilder` the chained call sits at line 1313, before the `TaskWorkflowPromptHandler` wrap at line 1326, so task-enabled workflows inherit it through `inner`. Neither `prompt_workflow` signature nor any `WorkflowPromptHandler` constructor arity moved.

7. **Ten tests where the gate reaches them.** All in `tests/skills_integration.rs` (`make test-skills` selector 3: **15 -> 25 passed**), never in `prompt_handler.rs`'s `mod tests`, which matches no selector and, being skills-gated, runs under no `--features "full"` leg either.

## Task Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | The opt-in flag and the shared prepend producer | `1ec2e340` | `src/server/workflow/prompt_handler.rs` |
| 2 | Honor the flag on BOTH `TaskWorkflowPromptHandler` branches | `58ba02d5` | `src/server/workflow/task_prompt_handler.rs` |
| 3 | Transcript tests where the quality gate can reach them | `8662f98b` | `tests/skills_integration.rs` |
| 4 | Builder-level reachability (GATE B) | `364477ec` | `src/server/builder.rs`, `src/server/mod.rs`, `tests/skills_integration.rs` |

## Files Created/Modified

**Modified**
- `src/server/workflow/prompt_handler.rs` — new `projected_skill_prepend: Option<String>` field, `with_projected_skill_prepend` setter, `projected_prepend` producer + skills-off twin, `handle` wiring (+142 / -1)
- `src/server/workflow/task_prompt_handler.rs` — the independent branch's header sequence (+22 / -2)
- `src/server/builder.rs` — `ServerCoreBuilder` field, setter, chained call before the task wrap (+63)
- `src/server/mod.rs` — `ServerBuilder` field, setter, chained call (+61)
- `tests/skills_integration.rs` — ten tests plus the `PrependTestTaskRouter` stub and the in-process harness (+629)

## Decisions Made

### GATE B — `add-builder-path` (LOCKED upstream at plan 126-01 Task 2; implemented here)

**What was built.** Both `ServerCoreBuilder` (`src/server/builder.rs`) and `ServerBuilder` (`src/server/mod.rs`) gained one `#[cfg]`'d, default-`false` `prepend_projected_skill` field, one `#[must_use]` chainable `with_workflow_skill_prepend(bool)` setter, and one chained `.with_projected_skill_prepend(self.prepend_projected_skill)` at `WorkflowPromptHandler` construction inside `prompt_workflow`.

**Why it exists — the problem it fixes.** Each `prompt_workflow` constructs its handler from its own clone of the workflow and exposed no hook to toggle a handler flag. A server registering a workflow the normal way could therefore not enable the D-04a prepend at all; the opt-in was reachable only via direct `WorkflowPromptHandler::new` / `::with_middleware_executor` plus `.prompt(name, handler)`. D-04's own rationale is that shipping the renderer without its consumer leaves the anti-drift claim theoretical — and an unreachable consumer is that same outcome renamed. With the builder path the claim holds **per server**, not merely per workflow value. This closes REVIEWS finding 2 (fable HIGH, gemini Finding 1) and threat T-126-25.

**Reversibility: one-way.** Two new public methods on published 2.x builder types. **Plans 126-06 and 126-07 inherit this**: 126-06's CHANGELOG entry should name `ServerCoreBuilder::with_workflow_skill_prepend` and `ServerBuilder::with_workflow_skill_prepend` alongside `WorkflowPromptHandler::with_projected_skill_prepend` as the three new public methods this phase adds, and 126-07's example should demonstrate the **builder** call (`.with_workflow_skill_prepend(true).prompt_workflow(wf)?`) rather than hand-constructing a handler — the builder path is what a real server uses, and demonstrating the hand-constructed route would document the gap this task closed.

**Ordering semantics, stated in both rustdocs.** `prompt_workflow` reads the field at REGISTRATION time, so the setter applies to workflows registered after it and leaves earlier ones alone. Both doctests show the correct order.

**Default stays off.** CONTEXT.md defers making the prepend default-on until the opt-in has real usage; this task does not touch that, and `server_core_builder_prompt_workflow_defaults_to_no_prepend` plus the negative half of `server_builder_prompt_workflow_reaches_the_prepend` pin it.

### Implementation decisions taken here

- **Cached `String`, not a `bool` (REVIEWS fable (b)).** All three measured reasons are in the field's doc comment: the D-15 slug `tracing::warn!` would re-fire on every `prompts/get`; message `[0]` would be re-derived from the handler's own clone while `skills/list` publishes a build-time snapshot, making two independently-derived strings that are *supposed* to be identical; and `handle` is a hot path.
- **A skills-off twin of `projected_prepend` rather than `#[cfg]`'d statements in `handle`.** The plan gated the producer on `skills` alone, which would have forced a `#[cfg]` branch into both handlers' statement sequences. An always-`None` twin keeps the flag-off path byte-identical *as code*, which is a stronger form of D-04's unchanged-execution property than a `#[cfg]`'d block approximating it. Carries `#[allow(clippy::unused_self)]` with the reason in its doc comment.
- **`prepend.is_some()` IS the flag on the task handler (REVIEWS codex).** No new accessor, no exposed field, one evaluation. `git diff src/server/workflow/prompt_handler.rs` in Task 2's commit is empty, as its acceptance criterion required.
- **The stub router's proof is the `_meta` assertion, not a comment.** `PrependTestTaskRouter::create_workflow_task` returns `Ok(json!({"task": {"taskId": "task-prepend-test-1"}}))` — the exact shape the extractor reads — and the test asserts the returned `GetPromptResult` carries that `task_id` in `_meta`. The delegating branch returns the inner result unchanged and carries no `_meta`, so the assertion is what distinguishes the branches. No additional router method needed overriding: `set_task_variables` and `complete_workflow_task` failures are warned, not propagated.
- **`ServerCore` has no prompt accessor**, so `server_core_builder_prompt_workflow_reaches_the_prepend` drives `ProtocolHandler::handle_request` directly, running the v1 `initialize` handshake first (a v1 core answers `-32002` to any earlier non-`initialize` request). Written inline rather than importing `tests/common/duplex.rs`, to keep the file self-contained.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocker] `#[cfg]` gate on `ServerCoreBuilder`'s new field widened to `all(feature = "skills", not(target_arch = "wasm32"))`**

- **Found during:** Task 4
- **Issue:** The plan's acceptance criterion specifies a bare `#[cfg(feature = "skills")]` for both new builder fields/setters. That is correct for `ServerBuilder`, whose entire struct is already `#[cfg(not(target_arch = "wasm32"))]` and whose `pending_skills` uses the bare gate. It is wrong for `ServerCoreBuilder`, which **does** compile on wasm32 while its only reader — `prompt_workflow` — is `#[cfg(not(target_arch = "wasm32"))]`. A bare gate would put a written-but-never-read field on a wasm32+skills build, which is a `dead_code` warning and therefore a hard error under the repo's `-D warnings` policy. Every existing skills member of that struct (`pending_skills` at `:142`, `.skill()` at `:463`, `.skills()`) already uses the `all(..)` form.
- **Fix:** `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` on `ServerCoreBuilder`'s field and setter; the bare gate retained on `ServerBuilder` exactly as specified. The chained call inside `prompt_workflow` uses the bare gate, which is correct there because `prompt_workflow` is already `not(wasm32)`.
- **Files modified:** `src/server/builder.rs`
- **Verification:** `RUSTFLAGS="" cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` exits 0; `--features "full"` and `--features "skills,..."` both exit 0; the acceptance criterion's real intent (`grep -c 'with_workflow_skill_prepend'` ≥ 2 in each file) is satisfied — both return 3.
- **Committed in:** `364477ec`

**2. [Rule 1 - Bug] Two rustdoc code-links adjacent to inline code failed `clippy::doc_link_code`**

- **Found during:** Task 1 (lint gate)
- **Issue:** `` [`SequentialWorkflow::as_skill`]`().body()` `` in the field doc and the setter doc both tripped `error: code link adjacent to code text` under `-W clippy::nursery`, failing the build for the crate lib with 2 errors.
- **Fix:** Reworded both to prose ("the body of the skill `as_skill` returns") preserving the same referent and link.
- **Files modified:** `src/server/workflow/prompt_handler.rs`
- **Verification:** the `full,skills` clippy leg exits 0.
- **Committed in:** `1ec2e340`

**3. [Rule 3 - Blocker] Test helper `register_tool` made generic over `T: ToolHandler`**

- **Found during:** Task 3
- **Issue:** The plan's harness sketch takes a `SimpleTool` parameter, but `SimpleTool<F>` is generic over its closure type, so a bare `SimpleTool` is `error[E0107]: missing generics`.
- **Fix:** `fn register_tool<T: ToolHandler + 'static>(..)` — simpler than naming the closure bound, and it makes the helper reusable for any handler.
- **Files modified:** `tests/skills_integration.rs`
- **Verification:** `--test skills_integration` compiles and runs 25 tests.
- **Committed in:** `8662f98b`

**4. [Rule 1 - Bug] `ServerBuilder` reachability test needed its tools registered before `prompt_workflow`**

- **Found during:** Task 4 (acceptance verification loop, first run)
- **Issue:** `server_builder_prompt_workflow_reaches_the_prepend` failed with `Internal("Tool 'orders_get' not found in registry")`. `prompt_workflow` snapshots the tool registry at registration time, and the test registered no tools — so `create_assistant_plan()?` (which still runs with the flag ON, by design) rejected the workflow before any transcript existed. The failure was the plan's own design working correctly; the test fixture was incomplete.
- **Fix:** Added a `with_fixture_tools(ServerBuilder) -> ServerBuilder` helper registering both fixture tools, applied to both the positive and negative constructions, with a comment recording the ordering requirement.
- **Files modified:** `tests/skills_integration.rs`
- **Verification:** 25 passed, 0 failed.
- **Committed in:** `364477ec`

**5. [Naming] Stub router spelled `PrependTestTaskRouter`**

- The plan names it `PrepandTestTaskRouter`, a typo. Used the correct spelling; the acceptance criterion pins the router's return payload and behaviour, not its identifier.

**Total deviations:** 4 auto-fixed (2 blockers, 2 bugs) plus 1 naming correction. **Impact:** none on the plan's semantics. Deviation 1 is a gate-correctness improvement over the plan's literal text; the rest are fixture and rustdoc mechanics.

## Verification Results

All plan-level `<verification>` commands re-run green after the final commit:

| Command | Result |
|---------|--------|
| `cargo build -p pmcp --features "full"` | exit 0 |
| `cargo build -p pmcp --features "skills,streamable-http,http-client,testing"` | exit 0 |
| `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` | exit 0 |
| `cargo test --features "skills,..." --test skills_integration` | **25 passed** (baseline 15, +10) |
| `cargo test --features "skills,full" --lib workflow` | 177 passed |
| `cargo test --features "skills,full" --lib server::builder` | 43 passed |
| `cargo test --features "full" --test v1_severability_tripwire` | 18 passed |
| `cargo test --features "full" --lib` | 2032 passed |
| `cargo test --features "skills,full" --lib` | 2193 passed |
| `cargo test --features "full" --test workflow_prompt_e2e_test` | 1 passed |
| `cargo test --features "full" --test v1_tasks_golden` | 14 passed |
| `make test-skills` | exit 0 — selectors 1/2/3/4 = 173 / 15 / 25 / 23 |
| `make lint-skills` (clippy `full,skills --lib --tests`) | exit 0 |
| `make lint` equivalent (clippy `full --lib --tests` + `--examples`) | exit 0 |
| `cargo fmt -p pmcp -- --check` | exit 0 |
| `pmat quality-gate --checks complexity` | **Total violations: 0** |

### The flag-off byte-identity proof, MEASURED

Not inferred from passing tests. A throwaway test serialized the DEFAULT (flag-off) `GetPromptResult` for a two-tool-step workflow, run twice: once at HEAD, once with `src/server/workflow/{prompt_handler,task_prompt_handler}.rs`, `src/server/builder.rs` and `src/server/mod.rs` checked out at the pre-plan commit `aae4d5c5`.

```
0479feef0481ebef90f9f347986309a86613afcde3500de24205bb84bbc18e16  transcript_base.json
0479feef0481ebef90f9f347986309a86613afcde3500de24205bb84bbc18e16  transcript_head.json
```

Identical SHA-256, `diff` empty, 1077 bytes each. The pre-plan sources were restored with `git checkout HEAD -- <paths>` and the throwaway test deleted; `git status --short src/ tests/` is clean.

### Anti-vacuity proven by injection, not asserted

Two defects were injected, observed red, and reverted:

| Injected defect | Test that reddened |
|---|---|
| Drop `messages.extend(prepend)` from the task handler's independent branch | `both_handlers_produce_the_same_message_zero` — "the two handlers disagreed on message [0]", left = user intent, right = projected body |
| Guard `create_assistant_plan()`'s CALL rather than its push | `flag_on_still_rejects_an_unregistered_tool` |

Both reverts confirmed by an empty `git diff --stat src/server/workflow/`.

## Known Stubs

None. `PrependTestTaskRouter` is a deliberate test double, not a production stub: it is confined to `tests/skills_integration.rs`, its unused trait methods return explicit `Err`s rather than silent successes, and the one method that succeeds returns the exact payload the branch selector reads.

## Threat Flags

None. This plan added no network endpoint, no auth path, no file access pattern and no schema change. The five threats the plan's register assigns `mitigate` (T-126-04, T-126-13, T-126-14, T-126-24, T-126-25) are each closed by a named test listed in the coverage block above.

## Issues Encountered

None outstanding.

Two environment notes, consistent with prior waves: `rtk` corrupts proxied `make`/`grep` output (it reported `EXIT=2` for a `make lint-skills` run whose underlying clippy invocation exits 0, and swallowed a `grep -v` pattern entirely), so every number in this SUMMARY was read through absolute binary paths — `/Users/guy/.cargo/bin/cargo`, `/usr/bin/grep`. No git hooks are installed in this checkout, so all quality gates were run manually before each commit.

## Next Phase Readiness

- **Plan 126-06 (docs/CHANGELOG)** inherits the GATE B decision recorded above. The CHANGELOG entry should name **three** new public methods, not one: `WorkflowPromptHandler::with_projected_skill_prepend`, `ServerCoreBuilder::with_workflow_skill_prepend`, `ServerBuilder::with_workflow_skill_prepend` — and state that the prepend is default-off and that the builder setting applies to workflows registered after the call.
- **Plan 126-07 (example)** should drive the opt-in through the **builder** (`.with_workflow_skill_prepend(true).prompt_workflow(wf)?`), not by hand-constructing a `WorkflowPromptHandler`. Demonstrating the hand-constructed route would document the very gap Task 4 closed.
- **The D-14 golden** (126-06) is unaffected: this plan renders no new bytes, it reuses `project()`'s output verbatim.
- **No blockers.** `SequentialWorkflow::instruction()`'s dead-value defect (`sequential.rs:168`) remains untouched and out of scope, as the plan's prohibition requires.

## Self-Check: PASSED

- All modified files present on disk and non-empty: `src/server/workflow/prompt_handler.rs`, `src/server/workflow/task_prompt_handler.rs`, `src/server/builder.rs`, `src/server/mod.rs`, `tests/skills_integration.rs`.
- All four task commit hashes resolve in `git log --oneline --all`: `1ec2e340`, `58ba02d5`, `8662f98b`, `364477ec`.
- Every task's `<acceptance_criteria>` was executed and passed, including the three grep-based gates (`as_skill` = 1 non-comment occurrence in `prompt_handler.rs` and 0 in `task_prompt_handler.rs`; `with_workflow_skill_prepend` = 3 in each builder file), the chained-call-before-task-wrap ordering check (line 1313 < line 1326), and the no-signature-change diffs.
- All plan-level `<verification>` commands re-run green after the final task commit, as tabulated above.
- Working tree clean of stray artifacts: the throwaway dump test was removed and the temporarily checked-out pre-plan sources restored.
