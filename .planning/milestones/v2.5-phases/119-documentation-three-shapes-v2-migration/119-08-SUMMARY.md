---
phase: 119
plan: 08
subsystem: documentation
tags: [docs, pmcp-book, protocol-era, tasks, transports, v2, provisionality]
status: complete

requires:
  - "119-05: pmcp-book/src/ch12-17-migrating-to-mcp-2026-07-28.md exists as the link target"
  - "119-01: HTTP-01..08 + CLNT-01/02/05 are [x]; the core v2 surface is settled, so no hedging"
provides:
  - "pmcp-book Chapter 12.7 — an additive, source-cited '## Era delta (v1 vs v2)' with a provisionality callout"
  - "pmcp-book Chapters 10 and 10.3 — era-disambiguation callouts and era-aware protocol-version lines"
  - "ch10-transports.md — one '> **Era note**' correcting the build-time/per-request statelessness conflation"
affects:
  - "119-10: the closing gate re-runs `make quality-gate` and both mdbook builds over these three files"
  - "any future plan that flips TASK-01..06 must revisit the provisionality callout in ch12-7-tasks.md"

tech-stack:
  added: []
  patterns:
    - "fenced-code sha256 digest as a mechanical 'no code block was touched' proof"
    - "callout-only repair: correct a stale taxonomy by adding scope, not by rewriting it"
    - "LINK-not-restate for the v2 story (Chapter 12.17 is the authority)"

key-files:
  created: []
  modified:
    - pmcp-book/src/ch12-7-tasks.md
    - pmcp-book/src/ch10-transports.md
    - pmcp-book/src/ch10-03-streamable-http.md

decisions:
  - "Cited both v2 Tasks examples WITH `--features full` — the plan's no-feature-flag criterion rested on a false premise (s51 does not compile under default features)"
  - "Corrected the plan's v2 create-response claim: v2 is a FLAT Task with `resultType: \"task\"`; `CreateTaskResult` is the v1 nested envelope"
  - "Booked NO requirements. DOCS-05 was already earned by 119-05 and is untouched"

metrics:
  duration: ~30 min
  completed: 2026-08-19

actuals:
  tokens: 2900
  tasks: 2
  commits: 2

requirements-completed: []
---

# Phase 119 Plan 08: Era Staleness Repair in the Tasks and Transport Chapters Summary

Three chapters that taught a v1-only story as if it were the whole story now name both
protocol eras where their readers already are: the Tasks chapter gains an additive,
source-cited v2 delta carrying the `tasks/list` security rationale and an explicit
provisionality callout, and the two transport chapters gain era callouts — including one that
corrects a behavioural conflation which would have led a reader to disable v1 sessions in
pursuit of a v2 property that is per-request and orthogonal.

## What Was Built

**Task 1 — `ch12-7-tasks.md` era delta** (`61671433`, +63 / −0). A vocabulary callout after
the H1, and a new `## Era delta (v1 vs v2)` immediately before `## Summary`, with five
subsections:

- **Where task support is negotiated** — a per-era table: v1 `capabilities.tasks` (legacy
  router path `experimental.tasks`) vs v2
  `capabilities.extensions["io.modelcontextprotocol/tasks"]`, with both negotiation
  directions. Source: `TASKS_EXTENSION_KEY`, `src/types/capabilities.rs`.
- **The create trigger changes, and each era ignores the other's signal** — added beyond the
  plan's list because it is the single most reader-surprising delta against the chapter's
  existing `## Capability Negotiation`, which teaches the per-request `task` field as *the*
  signal. On v2 that field is not consulted at all; the client's extension declaration on
  that request opens the create gate. Source: the era-trigger table on
  `maybe_build_task_created` and the predicate in `create_gate`, `src/server/task_dispatch.rs`.
- **Which `tasks/*` methods each era serves** — the five-row table. v1: `get`, `result`,
  `list`, `cancel`. v2: `get`, `update`, `cancel`, with `list` **and** `result` retired to
  `-32601`. Both retirements are stated, not just `tasks/list` (CONTEXT.md's D-07 summary
  names only the one). The `tasks/list` **security rationale is quoted in substance** — with
  no enumeration primitive a server cannot leak the existence of one caller's tasks to
  another — which appeared nowhere in the book before this commit.
- **The create response shape** — v1 nested `CreateTaskResult`, v2 flat with
  `resultType: "task"`. See deviation 2.
- **Runnable v2 examples** — `s50_v2_tasks_server` and `s51_v2_tasks_agent`. See deviation 1.

The provisionality callout sits at the head of the section: `ext-tasks` publishes only a
`draft/` schema, no versioned directory and no tagged release, so every v2 Tasks value is
provisional and must not be pinned. It points at the SDK's own `This value is PRE-FINAL` note
rather than at any internal record — `grep -cF '.planning/'` on the chapter is **0**.

**Task 2 — the two transport chapters** (`76bb3f97`, +8 / −2 across both files).

- Era-vocabulary callout at the head of each chapter, linking Chapter 12.17.
- Both `mcp-protocol-version` bullets now name `2025-11-25` (v1) **and** `2026-07-28` (v2).
- One `> **Era note**` at the head of `## Understanding Streamable HTTP Modes` carrying all
  three mandated clauses: the three modes are **v1 build-time** modes; **v2 statelessness is
  per-request and orthogonal** (a server on the *stateful* default config still mints
  sessions for v1 clients and still emits no session id on v2 — `s47_v2_stateless_mrtr`
  demonstrates exactly this, cited by full runnable invocation, and the note says plainly
  *do not reach for Mode 1 to "get" v2 statelessness*); and **`Last-Event-Id` resumption is a
  v1 mechanism**, which scopes the unqualified Mode-3 bullet without editing it.

## Deviations from Plan

### 1. [Rule 1 — Bug] Both v2 Tasks examples are cited WITH `--features full`

**Found during:** Task 1, running the plan's own acceptance build.

**The plan's premise is false.** It asserted that because neither example carries an
`[[example]]` block in `Cargo.toml`, both "build under DEFAULT features", and it made that a
prohibition plus two acceptance criteria (`grep -c 's5x… --features'` must equal `0`).
Auto-discovery is real; the feature conclusion does not follow. Measured:

| Invocation | Result |
|---|---|
| `cargo build --example s50_v2_tasks_server` (no flag) | ✅ exit 0 |
| `cargo build --example s51_v2_tasks_agent` (no flag) | ❌ **exit 101** — `error[E0433]: cannot find 'testing' in 'pmcp'` ×2, `src/lib.rs:63` `pub mod testing` is `#[cfg(any(test, feature = "testing"))]` and `default = ["logging", "v1-compat"]` |
| `cargo build --example s51_v2_tasks_agent --features full` | ✅ exit 0 (`full` includes `testing`) |

Both examples' own headers document `cargo run --example … --features full`. Writing the
no-flag form into a published chapter would have shipped a command that fails to compile —
precisely the class of defect this phase exists to remove — so the chapter carries
`--features full` for both, matching the examples' headers.

**Two acceptance criteria are therefore knowingly not met**
(`grep -c 's50_v2_tasks_server --features'` = 1, same for `s51`). They encode the false
premise. Everything they were protecting (a correct, runnable citation) is satisfied by the
form actually written.

**Consequence for a sibling:** plan **119-09**'s Task 2 verification row in
`119-VALIDATION.md` reads `cargo build --example s50_v2_tasks_server --example
s51_v2_tasks_agent…` — with no feature flag it will fail on `s51` for this same reason.
Flagged here so 119-09 or 119-10 does not rediscover it as a mystery. Not edited: both
`119-VALIDATION.md` and README are outside this plan's declared file scope.

### 2. [Rule 1 — Bug] The v2 create-response claim was wrong in the plan

**Found during:** Task 1, reading the cited source before writing.

The plan's action said "v2 task-augmented results use a task result type with
`CreateTaskResult`". `src/server/task_dispatch.rs:729-757` says the opposite:
`v1_create_result_value` builds `CreateTaskResult` — the **frozen nested** `{"task": …}`
envelope — while `v2_create_result_value` builds a **flat** `TaskV2::from_v1(task)` whose
discriminator `resultType: "task"` comes from `DispatchEnvelopeClaim::TASK_CREATED`
(asserted in-module at `:3704-3709`). Written as the source has it: v1 nested, v2 flat, both
carrying the related-task pointer under `_meta`. Had the plan's phrasing shipped, the book
would have described the v1 envelope as the v2 one.

### 3. [Scope note] `Last-Event-Id` count criterion rested on a wrong baseline

The plan required `grep -c 'Last-Event-Id' ch10-transports.md >= 4`, from a stated baseline
of "three pre-existing occurrences". The committed baseline is **2** (lines 257 and 577;
the third cited occurrence at line 250 is a `// Track events for resumption` code comment
that does not contain the token). Post-edit count is **3** — both pre-existing occurrences
preserved, one added by the era note. The substantive requirement is met; the arithmetic
criterion is not, because its baseline was off by one. No line was removed to reach this.

## Verification Results

| Verification | Result |
|---|---|
| `## Era delta` heading count in `ch12-7-tasks.md` | ✅ 1 |
| `io.modelcontextprotocol/tasks` / `tasks/update` / `32601` / `tasks/result` | ✅ 3 / 4 / 2 / 16 |
| `2026-07-28` in `ch12-7-tasks.md` (was 0) | ✅ 4 |
| `s50_v2_tasks_server` / `s51_v2_tasks_agent` cited | ✅ 1 / 2 |
| `2025-11-25T10:30:00Z` JSON timestamps preserved | ✅ 2 — not era-corrected (T-119-41) |
| `grep -cF '.planning/'` in `ch12-7-tasks.md` | ✅ 0 (T-119-43) |
| `grep -ciE 'TASK-0[1-6].*(complete\|final)'` | ✅ 0 (T-119-40) |
| Provisionality callout present and contains `draft` | ✅ |
| `git diff --numstat ch12-7-tasks.md` | ✅ **63 added / 0 removed** — purely additive |
| **Fenced-code sha256, `ch10-transports.md`** | ✅ `3c4b7cb3…8bdbdc` at HEAD **==** working tree (T-119-39) |
| **Fenced-code sha256, `ch10-03-streamable-http.md`** | ✅ `275fcc4c…84c7fe` at HEAD **==** working tree |
| `Era note` count in `ch10-transports.md` | ✅ exactly 1 |
| `2026-07-28` in ch10 / ch10.3 (both were 0) | ✅ 3 / 2 |
| `s47_v2_stateless_mrtr` cited in `ch10-transports.md` | ✅ 1 |
| Chapter 12.17 linked from both transport chapters | ✅ 2 / 1 |
| `grep -c '^## '` section counts | ✅ 18 and 6 — unchanged, no taxonomy rewrite (T-119-42) |
| `session_id_generator: None` count | ✅ 3 — unchanged |
| Removed-line caps (≤2 / ≤1) | ✅ 1 and 1 |
| `Last-Event-Id` in `ch10-transports.md` | ⚠️ 3, criterion said ≥4 — see deviation 3 |
| `cargo build --example s50_v2_tasks_server` (no flag) | ✅ exit 0 |
| `cargo build --example s51_v2_tasks_agent --features full` | ✅ exit 0 (no-flag form fails — deviation 1) |
| `cd pmcp-book && mdbook build` | ✅ exit 0, run after each task |
| `make quality-gate` | ✅ **exit 0** |

`make quality-gate` ran to completion in this worktree and exited 0. One caveat recorded
honestly: the captured log was truncated by the tee layer to 42 retained lines, so the
per-target detail was not inspectable after the fact — only the aggregate exit status was.
The exit code is the target's own aggregate signal (`lint-plans`, `fmt-check`, `lint`,
`build`, `test-all`, `pmcp-package-gate`, `audit` chained with `@$(MAKE)`), and the complete
diff for this plan is three markdown files with zero compilable content, so no gate leg can
be moved by it in either direction. `pmcp-course` was never built, so its
`mdbook-exercises` theme side effect did not fire.

## Requirements Ledger — nothing booked

`requirements-completed: []`, deliberately.

The plan frontmatter lists `requirements: [DOCS-05]`, but **DOCS-05 was already earned and
booked by plan 119-05**, which delivered all four of its clauses in Chapter 12.17 and
recorded its reasoning. `.planning/REQUIREMENTS.md:957` reads `- [x] **DOCS-05**` on entry to
this plan and is byte-identical on exit — this plan modified no file under `.planning/` other
than writing this SUMMARY. Re-booking a satisfied ID is a no-op at best and a ledger
corruption at worst, and the phase context names it as a defect 119-01 already hit and
reverted.

What this plan contributed is the *pointer target* of DOCS-05's Tasks clause — Chapter 12.17
§ `## Tasks on v2` says "the full era delta lives in Chapter 12.7", and as of `61671433` that
is true rather than aspirational. That is a completion of 119-05's cross-reference, not an
independent claim on the ID.

No state file was touched: `STATE.md` and `ROADMAP.md` are the orchestrator's to write after
the wave merges.

## Known Stubs

None. No placeholder text, no TODO/FIXME, no unwired reference. Every source citation in the
new prose (`src/types/capabilities.rs`, `src/types/tasks.rs`, `src/types/tools.rs`,
`src/types/mrtr.rs`, `src/server/task_dispatch.rs`) was read at HEAD before being written, and
every runnable command was executed — `s50` and `s51` as builds, `s47` cited in the form its
own header documents.

## Threat Flags

None. This plan created no network endpoint, no auth path, no file access pattern and no
schema change; the diff is markdown only. The four documentation-side threats it was written
against are each discharged with a mechanical check, recorded in the table above: T-119-39
(fenced-code digest equality, both files), T-119-40 (provisionality callout present,
TASK-0x-complete grep = 0), T-119-41 (both JSON timestamps survive), T-119-42 (section counts
pinned), T-119-43 (zero `.planning/` paths shipped). T-119-38 — a reader building a server
that leaks continuation state — is mitigated by the era note's third clause, which is present
and names `Last-Event-Id` resumption as v1-only.

## Self-Check: PASSED

Files verified present on disk:
- `pmcp-book/src/ch12-7-tasks.md` — FOUND (862 lines, was 799)
- `pmcp-book/src/ch10-transports.md` — FOUND (modified)
- `pmcp-book/src/ch10-03-streamable-http.md` — FOUND (modified)

Commits verified in `git log`:
- `61671433` — FOUND — `docs(119-08): add provisional v2 era delta to the Tasks chapter`
- `76bb3f97` — FOUND — `docs(119-08): add era callouts to the two transport chapters`

Worktree scope respected: `git diff --name-only <base>..HEAD` returns exactly the three
declared files. `pmcp-book/src/SUMMARY.md` (119-07's), `pmcp-course/` (119-06's), `README.md`
and `CHANGELOG.md` (119-09's), `docs/MIGRATION.md`, `pmcp-book/src/ch21-migration.md` and
`docs/v1-sunset-policy.md` are all untouched.
