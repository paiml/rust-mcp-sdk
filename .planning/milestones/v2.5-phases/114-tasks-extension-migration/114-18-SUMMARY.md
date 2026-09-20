---
phase: 114-tasks-extension-migration
plan: 18
subsystem: planning-bookkeeping
tags: [quality-gate, requirements, spec-recheck, deferred-items, docs, d-18-hold]
requires:
  - "114-01 vendored ext-tasks schema + D-18 hold record"
  - "114-03..114-17, 114-19, 114-20 (every implementation plan this gate grades)"
  - "114-20 owner contract-first decision (option-b)"
provides:
  - "TASK-01..06 booked [~] implemented; pending final schema"
  - "114-SPEC-RECHECK.md finalized: 40 rows, every one walkable to a landing identifier"
  - "deferred-items.md: 25 unique IDs, every item owned or explicitly unowned"
  - "one green whole-tree gate run with every number measured against a phase-base manifest"
affects:
  - ".planning/REQUIREMENTS.md"
  - ".planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md"
  - ".planning/phases/114-tasks-extension-migration/deferred-items.md"
  - "src/types/tasks.rs, src/types/tools.rs, src/server/{task_store,tasks,mod,builder}.rs, src/client/mod.rs, crates/pmcp-tasks/src/constants.rs (doc comments only)"
tech-stack:
  added: []
  patterns:
    - "base-commit manifest measured in a detached worktree with its own CARGO_TARGET_DIR, asserted as deltas rather than against hard-coded totals"
    - "doc-only sweep proven doc-only by a non-comment-diff-line assertion"
key-files:
  created:
    - ".planning/phases/114-tasks-extension-migration/114-18-SUMMARY.md"
  modified:
    - ".planning/REQUIREMENTS.md"
    - ".planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md"
    - ".planning/phases/114-tasks-extension-migration/deferred-items.md"
    - "crates/pmcp-tasks/src/constants.rs"
    - "src/types/tasks.rs"
    - "src/types/tools.rs"
    - "src/server/task_store.rs"
    - "src/server/tasks.rs"
    - "src/server/mod.rs"
    - "src/server/builder.rs"
    - "src/client/mod.rs"
decisions:
  - "DQ7 cited, not decided: the contract-first waiver is the owner's (option-b, Guy Ernest, 2026-07-28); 114-18 confirmed its follow-up row exists and its residual costs are unchanged by measurement"
  - "resultType:\"task\" ruled CONFORMANT-BY-EXTENSION, not prospective drift — the published core's ResultType union carries an open `| string` tail and the extension is what names the value"
  - "Phase 112's absent-resultType-means-complete decoding is the CONTRACT, not a tolerance — the published core makes it a client MUST; the 2026-07-29 advance observation is withdrawn"
  - "TASK-05 booked with the D-07 row-3 qualification carried into REQUIREMENTS.md rather than absorbed"
  - "the ledger's colliding deferral IDs resolved by redirect table, NOT by rewriting landed SUMMARY files"
metrics:
  duration: "~2h"
  completed: 2026-08-01
  tasks_completed: 4
  tasks_total: 4
  status: complete
  checkpoint: "Task 4 checkpoint:human-verify gate=blocking — APPROVED by Guy Ernest (owner) 2026-08-01; closes the sign-off ONLY, the D-18 publication hold is untouched"
---

# Phase 114 Plan 18: Phase Gate, Requirement Booking & Ledger Close-Out Summary

**One-liner:** The whole-phase gate runs green at 294/4899 with every number measured against a
base-commit manifest rather than a remembered total, all six TASK requirements are booked `[~]` under
a hold whose 40 inventory rows now walk to real identifiers, and the deferred ledger has 25 unique
IDs where three of them used to collide.

**Status: COMPLETE — 4/4 tasks.** Task 4's `checkpoint:human-verify gate="blocking"` was returned to
the orchestrator unanswered rather than self-approved, and was **approved by Guy Ernest (owner) on
2026-08-01** with no changes requested. **That approval closes the sign-off checkpoint and nothing
else: TASK-01…06 remain `[~]`, `## Verdict` remains `PENDING`, and Phase 114 remains `[~]` because
the D-18 publication hold is still engaged.** See § *Task 4*.

---

## Precondition: the plan's own STOP clause fires as a FALSE POSITIVE, and here is the measurement

Task 2's acceptance criteria carry a stop condition:

> **No `## BLOCKING: TASK-05 security defect` heading exists in `114-15-SUMMARY.md`.** If one does,
> this plan STOPS.

**Evaluated literally by grep, it fires. It is a false positive, and the plan text is defective —
it tests for heading EXISTENCE where it must test heading CONTENT.** Four measurements, taken before
any work began:

1. **The heading DOES exist** — `114-15-SUMMARY.md:359`.
2. **Its body reads `**NONE FOUND.**`** — *"All three v2 `tasks/*` methods are closed to a
   cross-caller over a real socket, the refusals are indistinguishable from an absent id on both code
   and message, and no refusal performed its write anyway."* It goes on to state explicitly:
   *"there is no such defect, so 114-18 is not blocked by this plan."*
3. **`114-15-PLAN.md:177-181` gives two orders, and 114-15's executor obeyed both.** The heading is
   mandated **conditionally** — *"any production defect found here MUST be written into the SUMMARY
   under a heading `## BLOCKING: TASK-05 security defect`"* — while the very next sentence orders,
   unconditionally, *"State that obligation in this plan's SUMMARY explicitly, naming `114-18`, so
   the block is discoverable from the artifact rather than only from a reviewer's memory."* Emitting
   the heading with a NONE-FOUND body satisfies the second without triggering the first.
4. **Independent corroboration that no defect exists:**
   `git diff --stat c05f562d~1..98d34bb5 -- src/ crates/ Cargo.toml Cargo.lock` for 114-15's full
   commit range is **byte-EMPTY**. The range touched five files: `ROADMAP.md`, `STATE.md`,
   `114-15-SUMMARY.md`, `deferred-items.md`, `tests/v2_tasks_security.rs`. **Nothing was fixed
   because nothing needed fixing.**

**The precondition's INTENT — "no unresolved TASK-05 security defect blocks sign-off" — is
SATISFIED.** TASK-05 is booked and the sign-off checkpoint runs.

**The defect is in the plan text and is fixed at the source rather than re-litigated by the next
reader:** a plan that wants "a defect blocks sign-off" must grep for the defect, not for the word.
The correct predicate is *"the heading exists AND its body does not begin `NONE FOUND`"*, or better,
*"`deferred-items.md` carries a production-defect entry attributed to 114-15"* — which it does not.
This is the **tenth** measured plan-text defect in Phase 114.

---

## Phase base manifest

Measured at the phase's base commit **`27364eb1`** (*"docs(114): replan with cross-AI review feedback
— 20 plans, 12 waves"*, 2026-07-27T21:40:40-07:00) in a **detached worktree** at
`…/rust-mcp-sdk-114base` with its **own `CARGO_TARGET_DIR`**, so the main tree's warm `target/` was
neither read nor written. The worktree was removed after the run.

**The manifest exists because the plan predicted the failure it prevents:** repository-wide constants
age across an 18-plan phase and produce false findings. Where a planning-time number disagrees with
the manifest, **the manifest wins and the difference is recorded, not treated as a regression.** Two
of the four planning-time numbers were wrong.

| Gate | at base `27364eb1` | at HEAD | delta | verdict |
|---|---|---|---|---|
| `cargo semver-checks check-release` (baseline = published crates.io 2.17.0) | exit **100** — 223 checks: **222 pass, 1 fail**, 30 skip | exit **100** — **222 pass, 1 fail**, 30 skip | **0** | **identical.** Planning-time "223/223" was measured with a DIFFERENT baseline — see D-114-W |
| `cargo semver-checks check-release --baseline-rev 27364eb1` | — | exit **0** — **223 checks: 223 pass**, 30 skip, *no semver update required* | — | **this is the phase's own result: zero semver movement** |
| `cargo public-api --simplified` item count | **21 324** | **21 689** | **+365** | **0 REMOVED**, 365 ADDED, all attributed |
| `pmat analyze complexity --max-cognitive 25` (pmat 3.15.0) | **4** violations, **0 in `./src/`** | **5** violations, **0 in `./src/`** | **+1** | the addition is 114-13's tripwire TEST at cog 33 |
| `make wasm-build` warnings | **86** (exit 0) | **91** (exit 0) | **+5** | all `dead_code` on the wasm target |
| `cargo doc --no-deps --features full` warnings | **28** | **28** | **0** | per-file distribution **byte-identical** |
| `make test-feature-flags` | exit **2**, **49** `^error` lines | exit **2**, **62** | **+13** | pre-existing (D-114-E); the delta is this phase's — D-114-U |
| `make doc-check` | exit **2**, **26** errors | exit **2**, **26** errors | **0** | error-header sets **byte-identical** — D-114-V |

**The single semver failure is the same at both commits:** `type_marked_deprecated` — `#[deprecated]`
added on `Struct OptimizedSseTransport` (`src/shared/sse_optimized.rs:95`). It predates Phase 114 and
is a *correct* report: the published 2.17.0 did not carry that attribute.

---

## Task 1 — the stale-doc sweep

**Doc-only, and proven so rather than asserted.** The commit's own assertion:

```
git diff -U0 -- src/ crates/ | grep '^[+-]' | grep -v '^(\+\+\+|---)' \
  | grep -vE '^[+-]\s*(///|//!)' | grep -vE '^[+-]\s*$'
→ grep exit 1 (NO non-comment added or removed lines)
```

`cargo fmt --all -- --check` exit 0; `cargo check --features full` exit 0.

### The two known-false rustdocs: VERIFIED corrected, not trusted

The task ordered verification rather than belief, citing 113-29's recorded failure class (a stale
*"deliberately does NOT do X"* comment after X became true). Both were already fixed:

- **`is_v1_task_era`** — `grep -c "gates ONLY the" src/server/task_dispatch.rs` → **0**. 114-08 not
  only removed the false sentences but wrote the falsification into the doc block itself: *"This
  block previously claimed the predicate gated only the `-32002` emission and that `tasks/list` was
  unchanged on every era. Both sentences were falsified by plan 114-08 and are rewritten in the same
  commit that falsified them."*
- **`V2_TASKS_NOT_NEGOTIATED`** — the constant **no longer exists**. It was REPLACED by
  `V2_TASKS_METHOD_RETIRED`, and the only surviving textual occurrence is a heading explaining the
  replacement (`task_dispatch.rs:108`).

### A second measured plan-text defect, on this task's own criterion

> `grep -rn "advertises no" src/server/task_dispatch.rs` returns nothing

**It returns THREE hits (lines 1981, 2018, 2106), and all three are TRUE.** They are 114-09's
sentences about a **backendless server** — *"Cases 3 and 4 are SKIPPED for a backendless server: it
advertises no tasks extension at all, so telling such a caller to declare one — or to authenticate —
would send it to fix the wrong thing (T-114-33)."* The criterion was aimed at
`V2_TASKS_NOT_NEGOTIATED`'s claim that *pmcp* advertises no tasks entry; that constant is gone.

**A bare-substring criterion cannot distinguish a false claim from a true one that shares four
words.** Intent satisfied; the literal test is unsatisfiable and is recorded rather than worked
around. Eleventh measured plan-text defect.

### The auditable sweep

| grep term (doc comments in `src/` + `crates/`) | hits REVIEWED | doc lines CHANGED |
|---|---|---|
| `tasks/list` | 40 | 16 |
| `tasks/result` | 60 | 24 |
| `experimental.tasks` | 7 | 1 |
| `capabilities.tasks` | 13 | 1 |
| **total** | **120** | **42** |

**23 sweep sites changed across 7 files.** `capabilities.tasks`'s 13 hits were all already
era-qualified by 114-05 and 114-19 — one changed line, and it is a cross-reference rather than a
correction. The one genuinely false FAMILY was the era-unqualified claim *"the SDK then serves
`tasks/get`, `tasks/result`, `tasks/list`, and `tasks/cancel`"*, present at **six** sites
(`types/tools.rs`, `server/task_store.rs` ×2, `server/mod.rs`, `server/builder.rs` ×2) plus the
`TaskRouter` trait.

**What changed, by mandate:**

- `crates/pmcp-tasks/src/constants.rs` — `METHOD_TASKS_LIST` and `METHOD_TASKS_RESULT` are marked
  **v1-only** and each names the `-32601` v2 answer and its inventory row (37/38);
  `MODEL_IMMEDIATE_RESPONSE_META_KEY` states it has **no v2 counterpart** (the v2
  `CreateTaskResult` is a flat `Result & Task` with no slot for a provisional model answer);
  `METHOD_TASKS_STATUS_NOTIFICATION` states the v2 push surface is `notifications/tasks`, a **MAY**
  this phase declines. A module-level § *Era split* names the whole set once.
- `src/types/tasks.rs` — `Task`, `CreateTaskResult`, `GetTaskResult`, `CancelTaskResult` and
  `ListTasksResult` each name themselves as the v1 wire shape and point at their v2 counterpart
  (`TaskV2` / `TaskDetailV2`) or record that there is none. `Task`'s block says plainly that
  *serializing a `Task` onto a v2 response is a schema-invalid answer*. `TaskPollDecision`'s two
  *"issue a separate `tasks/result` call"* sentences are made era-dependent.
- `src/server/tasks.rs` — `handle_tasks_result` and `handle_tasks_list` are marked v1-only on the
  trait; `task_capabilities` is marked as the v1 spelling.

### An out-of-plan finding this task caught, and the reason it existed

**Measuring against the base manifest found two NEW `cargo doc` warnings that no gate could see.**
HEAD carried **30** warnings against the base's **28**; the two extras were both in
`src/client/mod.rs` — `[`Error::Parse`]` and `[`Error::Capability`]`, **neither of which is a variant
of `Error`**. They were introduced by 114-19, whose own `make quality-gate` was green, because
**`make quality-gate` does not run `doc-check`.**

Fixed here (Rule 1 — a doc link naming a nonexistent variant is a wrong doc, and this task owns
docs): `Error::Protocol` carrying `ErrorCode::PARSE_ERROR` (which is what `Error::parse` builds) and
`Error::UnsupportedCapability` (which is what `Error::capability` builds). After the fix the warning
count is **28** and the per-file distribution is **byte-identical to base**.

The gate gap itself is filed as **D-114-V**, with the cheap separable fix named: a `cargo doc`
warning-count tripwire, or `doc-check` restricted to `unresolved_link` — not the whole 26-error
pre-existing `private_intra_doc_links` population, which would block every commit in the repo.

**Commit:** `6be9f5fe`

---

## Task 2 — the whole-phase gate

Every number below is measured. Where the plan quoted a planning-time constant, both the constant and
the measurement are given.

### 1. `make quality-gate` — **exit 0**

| metric | value |
|---|---|
| test-result lines | **294** |
| passed | **4899** |
| failed | **0** |
| ignored | **81** |
| non-`ok.` result lines | **0** |
| D-114-A keychain flakes (`no native root CA certificates found`) | **0** |
| truncation markers | **0** |
| `^warning` lines | **0** |
| log length | 8310 lines, captured through `/usr/bin/make` |

**294/4899 is BYTE-IDENTICAL to 114-16 and to 114-17, and that identity IS the check.** This plan
adds no test binary and no lib test and changes no behaviour, so any movement would have been
something else's. Run with `RUST_TEST_THREADS=1` per D-114-A addendum 3 (`make test-unit` reads
`RUST_TEST_THREADS`, not `NEXTEST_TEST_THREADS`); disk at 92%, 73 GiB free.

### 2. `make lint` — **exit 0, 0 warnings, 0 errors**

Run standalone as well as inside the gate. `make lint` clippies `--lib --tests` with the pedantic +
nursery + cargo groups, then `RUSTFLAGS="-D warnings" cargo check --features full --examples`.

### 3. `cargo semver-checks` — **223/223, no update required, against the phase base**

See § *Phase base manifest*. The planning-time "223/223" and the bare `check-release` form's 222/223
are **both true and answer different questions**; the conflation is filed as **D-114-W**. A plan
asserting a semver ratio must name its baseline.

### 4. `cargo public-api` — **0 REMOVED, 365 ADDED, every one attributable**

| added surface | count | owning plan |
|---|---|---|
| `TaskV2` / `TaskDetailV2` / `DetailedTaskV2` / `DETAIL_KEY_*` | 266 | 114-11 |
| `TaskInputDelivery`, `TaskInputSnapshot`, `task_input_snapshot`, `supports_inputs`, `record_input_requests`, `deliver_task_inputs`, `get_error`, `set_error`, `kind_of`, `partition_input_delivery`, `outstanding`, `is_complete` | 58 | 114-04 |
| `TasksExtensionCapability`, `TASKS_EXTENSION_KEY`, `ClientCapabilities::extensions` | 23 | 114-03 |
| `Client::{tasks_update, tasks_get_detailed, tasks_cancel_ack, wait_for_task_with_inputs}`, `ClientBuilder::with_tasks_extension`, `WaitForTaskOptions`, `InputRequests`/`InputResponses`/`InputRequestKind` | 15 | 114-19 |
| `EXT_TASKS_SCHEMA_COMMIT` (3 re-export paths) | 3 | 114-01 |

**No public item nobody planned.** The remainder are additional trait impls on existing types
(`TaskStoreError`, `TaskStatus`, `Task`, `Result`, `CallToolResult`).

### 5. `cargo nextest run -p pmcp-tasks` — **exit 0, 514/514 passed**

```
git diff --stat 27364eb1..HEAD -- crates/pmcp-tasks/tests/
 crates/pmcp-tasks/tests/input_delivery.rs | 1472 +++++++++++++++++++++++++++++
 1 file changed, 1472 insertions(+)
```

**Exactly ONE file, created by 114-07**, as the criterion requires.

### 6. `make test-feature-flags` — **exit 2. Pre-existing (D-114-E), and the criterion is unsatisfiable as written.**

Measured at the phase base as well as at HEAD, which D-114-E had only done at 114-07's base:

| | exit | `^error` lines | `mrtr.rs` | `subscriptions.rs` | `core.rs` | `task_dispatch.rs` | `protocol_helpers.rs` | `protocol/mod.rs` | `server/mod.rs` | `sse_parser.rs` |
|---|---|---|---|---|---|---|---|---|---|---|
| base `27364eb1` | **2** | **49** | 36 | 7 | 2 | 0 | 0 | 0 | 1 | 2 |
| HEAD | **2** | **62** | 39 | 7 | 4 | 6 | 1 | 1 | 1 | 2 |

Same failing row as D-114-E: row 1/4, second sub-command,
`cargo clippy -p pmcp-tasks --no-default-features -- -D warnings`, exit 101. **Zero errors are in
`crates/pmcp-tasks/`.** The compile claim the four rows exist to make is **GREEN**: all five
`cargo check -p pmcp-tasks` rows exit **0**.

**The acceptance criterion "`make test-feature-flags` exits 0 for all four rows" was unsatisfiable at
the moment it was written** — the target was already red at the phase base. This is the second plan
in the phase to carry it. The **+13 delta is this phase's**, attributed symbol by symbol to
114-05/06/13/14, and is filed as **D-114-U** so it is not absorbed by D-114-E's "pre-existing"
wording. Not fixed here: 13 `#[cfg]`/`allow` decisions across five root-`pmcp` files owned by four
other plans is neither doc-only nor reviewable inside a bookkeeping plan, and an `allow` is not a
neutral edit — it also hides the next real dead item.

### 7. `make wasm-build` — **exit 0**, 86 → **91** warnings

All `dead_code` on the wasm target: `src/types/mrtr.rs` +3, `src/shared/protocol_helpers.rs` +1,
`src/types/protocol/mod.rs` +1 — the same class as the pre-existing 37-strong `types::mrtr` dead
block D-14 predicted, and the same symbols as D-114-U.

### 8. `pmat analyze complexity --max-cognitive 25` — **base 4, HEAD 5, ZERO in `./src/` at both**

| # | rule | site | value |
|---|---|---|---|
| 1 | cyclomatic | `crates/mcp-tester/tests/property_tests.rs:53 prop_g3_handler_detection_independent_of_sdk` | 22 |
| 2 | cognitive | same function | 29 |
| 3 | cognitive | `crates/pmcp-server-toolkit/tests/sql_server_http_example.rs:158 example_body_is_at_most_15_lines` | 28 |
| 4 | cognitive | `crates/pmcp-agent/tests/http_sources_mock.rs:80 read_request` | 28 |
| 5 | cognitive | **HEAD only** — `tests/v2_tasks_update_routing.rs:1081 no_source_site_routes_tasks_update_through_the_mrtr_ingress` | **33** |

**No fourth `src/` violation, because there is no first one.** #5 is 114-13's tripwire TEST, already
attributed by `114-14-SUMMARY.md`; CLAUDE.md's CI gate filters `select(.path | startswith("src/"))`,
so it does not breach it.

**A correction to STATE.md, recorded in D-114-W.** STATE.md says *"the gate at 3 pre-existing
violations, including D-113-U (`write_canonical` cog 26, unowned)"*. With the CLAUDE.md-pinned
**pmat 3.15.0**, `write_canonical` appears in **neither** violation list. **That is a fact about the
instrument, not about the obligation** — D-113-U still needs an owner before this branch merges, and
`deferred-items.md` § *Inherited from Phase 113* says so explicitly.

### 9. The example pair — **`s51` exits 0, all five demonstrations behaved as documented**

`cargo run --features full --example s50_v2_tasks_server` served the whole session;
`… --example s51_v2_tasks_agent` exited **0**. Verbatim from the transcript:

- **[1] Negotiation** — `server/discover` returns `extensions: ["io.modelcontextprotocol/tasks"]`.
- **[2] Autonomous round trip** — created `70e1413d…` at `input_required`, server asked *"Which topic
  should I research?"*, agent answered, **input rounds: 1**, terminal `completed` with the result
  inlined and `ttl Some(300000) ms`.
- **[3] Manual update** — `tasks_get_detailed` → `outstanding: ["topic"]` → `tasks/update`
  (*"the ack is an EMPTY object"*) → result.
- **[4] Undeclared client** — ordinary `CallToolResult`, **no** `_meta.…/related-task`; `tasks/get`
  answers **-32021** naming the exact `_meta` path to send.
- **[5] Retirements** — client refuses `tasks/list`/`tasks/result` **locally**, and a raw
  `send_raw` frame per method shows the **server** answering **-32601**.

### 10. `cargo +nightly fuzz run fuzz_tasks_update -- -runs=20000 -seed=114018` — **exit 0**

*"Done 20000 runs in 1 second(s)"*. **Artifacts dir EMPTY (0 files).**

### 11. Dependency diffs — **EMPTY**

```
git diff --stat -- Cargo.toml Cargo.lock crates/pmcp-tasks/Cargo.toml            → (empty)
git diff --stat 27364eb1..HEAD -- Cargo.toml Cargo.lock crates/pmcp-tasks/Cargo.toml → (empty)
```

**Zero new runtime dependencies across the entire phase**, byte-exact. The only manifest movement is
`fuzz/Cargo.toml` +13 lines — a `[[bin]]` block for 114-14's fuzz target, which is not a runtime
dependency of any published crate. This closes **T-114-96** and **T-114-SC**.

### DQ7 — the contract-first question, CITED not decided

`114-CONTRACT-DECISION.md` § 4 has a filled `## Decision`:

| field | value |
|---|---|
| Chosen | **option-b** |
| Decided by | **Guy Ernest (owner)** |
| Date | **2026-07-28** |
| Follow-up obligation | present in `114-SPEC-RECHECK.md` § *⚠ Carried obligation — the Phase-114 contract-first waiver* — **confirmed by reading the row**, not assumed |

`make comply` — **exit 0**. Its output confirms the decision record's §1.5 measurement *by
re-measurement*: `pmat comply check --path .` reads the **in-repo, git-tracked** `contracts/` tree —
**CB-1200** finds 2 contract files, **CB-1202** 2/2 critical keywords covered (100%), **CB-1205**
provability invariant satisfied, **CB-1305** 2/2 classified. `comply-bindings-check` resolves all
four team-servers bindings.

**The residual costs the owner accepted are present and unchanged, as the waiver row predicted:**
**CB-1207** still reports *1/2 contract(s) stale (>90 days)*. A re-runner should expect that and must
**not** read it as drift.

> **A measured plan-text defect in Task 4's own checkpoint script, corrected in the sign-off below.**
> `114-18-PLAN.md`'s Task 4 asks the human to confirm *"DQ7 (no contract YAML — `../provable-contracts/`
> is absent and `make comply` is repo-local and informational)"*. **That framing was falsified before
> execution and must not be presented to the owner.** `114-CONTRACT-DECISION.md` §1.5 measured the
> premise and found it FALSE: `contracts/` is in-repo, git-tracked and already graded. The absent
> `../provable-contracts/` holds the `pv` CLI and `proof-status.json`, not the authoring destination.
> **Option-b rests SOLELY on the D-18 provisional-values argument** — a contract authored now would
> pin 40 values this gate expects to move. A future reader may **not** cite this waiver as precedent
> for *"there was nowhere to write it."* Twelfth measured plan-text defect.

**Commit:** `9b7d9a01`

---

## Task 3 — booking, hold record, ledger

### (a) `.planning/REQUIREMENTS.md`

**TASK-01…TASK-06 are `[~]` with the exact `— *implemented; pending final schema*` qualifier the
CLNT rows use. None is `[x]`.** All six traceability rows read *Implemented — pending final schema*.
A section header points at `114-SPEC-RECHECK.md` so a reader of REQUIREMENTS.md alone finds the
obligation, and the status-marker legend now distinguishes the **two different gates in play** —
113's for HTTP-0x/CLNT-0x, 114's DQ6 both-repositories trigger for TASK-01..06. That distinction did
not exist before and is exactly the confusion DQ6 was written to prevent.

**TASK-05 carries its scope qualification into the booking, as the recheck record's ⚠ row obliges.**
The booking states that "fails closed" applies to **auth-configured deployments** — where a caller
with no subject is refused `-32003` — while on a server with **no auth provider at all** D-07 row 3
deliberately maps every anonymous caller onto one `ANONYMOUS_PRINCIPAL` bucket. That is a
development/stdio affordance, **not** per-caller isolation; D-07 is LOCKED and is not reopened; and
it is independently bounded by `TaskSecurityConfig::default()`'s `allow_anonymous: false`. **TASK-05
is never recorded as delivering more isolation than it does.** The no-cross-caller-visibility half is
proven by `tests/v2_tasks_security.rs` over a real socket.

**TASK-04 carries the `resultType:"task"` judgement, made explicitly rather than absorbed** — see
§ *Decisions* below.

### (b) `114-SPEC-RECHECK.md` — finalized

**Every one of the 39 rows resolved from a FILE to the IDENTIFIER that carries the value.** A new
§ *Landing sites* table makes `## Procedure` step 3 a mechanical walk. **No row was blank and no row
was stale.**

Highlights a re-runner needs: row 1 → `TASKS_EXTENSION_KEY` (`capabilities.rs:346`); rows 4-10 →
`TaskV2`'s seven fields, projected only by `TaskV2::from_v1`; rows 11-15 → `TaskStatus`, set-equality
locked at runtime; rows 16-17 → `v2_create_result_value` + `DispatchEnvelopeClaim::TASK_CREATED`;
row 26 → `INPUT_RESPONSES_KEY`; row 29 → `INVALID_PARAMS` + the single `V2_TASK_NOT_FOUND_MESSAGE`;
row 34 → `TASK_NAME_BEARING_METHODS`, **a table deliberately separate from `MRTR_METHODS`**.

**Row 36 is the only cell with no identifier, and that is the correct state**, not a gap: the push
surface is a spec MAY this phase declines. An empty cell there means *declined*, never *forgotten* —
now also filed as **D-114-X**.

**Row 40 ADDED** for the `input_required` axis overlap the 2026-07-29 run asked for by name. Row
numbering now stops at 40 and the carried-obligation row's *"stops at 39"* sentence is updated.

`## Verdict` stays **`PENDING`**. The three-branch `## Third Outcome Policy` and DQ6's *"BOTH. Not
either."* condition are intact and untouched.

### The re-measurement, with a timestamp

**`2026-08-01T00:09:19Z`**, taken with the **prescribed `gh api … --jq` form** — so the 2026-07-29
run's METHOD CAVEAT (listings read over plain HTTP because no shell was available) is **DISCHARGED**.

| Repository | Versioned dirs | Condition |
|---|---|---|
| `modelcontextprotocol/modelcontextprotocol` | `2024-11-05`, `2025-03-26`, `2025-06-18`, `2025-11-25`, **`2026-07-28`**, `draft` | **met** |
| `modelcontextprotocol/ext-tasks` | `draft` only — **0 tags, 0 releases**, `specification/` = `draft` only, `schema/draft` still at `29f83d5` (2026-05-22) | **NOT met** |

**Landing: `STILL-ABSENT`** (partial publication, rule 5). Steps 2-3 **not executed, deliberately**.
**No inventory row is marked CONFIRMED by this plan.**

**`114-01`'s provenance tripwire EXECUTED** — the 2026-07-29 gap, closed:
`binary_id(pmcp::vendored_schema_provenance)` → **5 tests run: 5 passed, 0 skipped**, and
`shasum -a 256` on both vendored files equals `PROVENANCE.md` byte for byte. **Unchanged by test, not
by inference.**

> **A selector correction, measured.** The amendment predicted `-E 'test(/vendored_schema/)'` would
> *"select 0 or pass vacuously"*. **It does not.** All five provenance tests happen to be named
> `vendored_schema_*`, so the name matcher selects **6** — all five, **plus**
> `pmcp::v2_tasks_tripwires::the_task_status_wire_strings_are_set_equal_to_the_vendored_schema` from
> a **different binary**. So it **over**-selects here rather than under-selecting. The general trap
> (a name matcher is not a binary matcher) stands; the specific prediction did not. `binary_id(...)`
> selects exactly the five.

**The remaining trigger is now a ONE-repository check**, stated precisely in three places
(REQUIREMENTS.md, the recheck record, D-114-S):
`gh api repos/modelcontextprotocol/ext-tasks/contents/schema --jq '.[].name'`.

### (c) `deferred-items.md` — 25 unique IDs where three used to collide

**MEASURED, not suspected:** `grep -n "^## "` showed **`D-114-M` used three times** and **`D-114-N`
twice**. `D-114-P`'s *"Related: D-114-M (114-14)"* therefore pointed at an ambiguity rather than an
entry, which defeats the point of an ID.

Resolved by keeping each ID for whichever entry existing documents already cite, renumbering the rest
into free letters, and publishing the map:

| Old | Filed by | Subject | New | Cited by the old ID? |
|---|---|---|---|---|
| `D-114-M` | 114-13 | `handle_tasks_update` default answers `-32603` | **kept** | yes — `114-13-SUMMARY.md:240,366` |
| `D-114-M` | 2026-07-29 spec run | published core schema not vendored | **`D-114-R`** | **no** |
| `D-114-M` | 114-14 | a `TaskRouter` decodes `tasks/update` unaided | **`D-114-T`** | **yes** — `114-14-SUMMARY.md:49,418`, `D-114-P`, `STATE.md` |
| `D-114-N` | 2026-07-29 spec run | nothing watches `ext-tasks` | **`D-114-S`** | **no** |
| `D-114-N` | 114-14 | store without inputs → `TASKS_NOT_ENABLED` | **kept** | yes |

**Landed SUMMARY files are NOT rewritten.** Rewriting a landed artifact to hide an inconsistency is
worse than a redirect; each renamed entry carries the note at its own heading. `D-114-R` was free —
`114-16-SUMMARY.md` records it only as a *corrected commit-message typo*, never as an assigned ID.

**`D-114-M` was also found still OPEN.** It was owned by 114-14, which added the router branch but
did **not** change the default: `src/server/tasks.rs`'s `handle_tasks_update` still returns
`Error::internal(…)` → `-32603`. Reassigned to **Phase 118**, beside `D-114-P` and `D-114-T`, which
are the same question about what a `TaskRouter` owes a v2 caller.

**Every class the plan named is accounted for**, via a § *Ledger completeness sweep* table:

| class | where |
|---|---|
| every `114-*-SUMMARY.md` finding / defect-not-fixed | `D-114-A` … `D-114-W` individually |
| server-side `Mcp-Name` enforcement (DQ4, OFF) | **`D-114-C`** — already filed, owner Phase 118 |
| `notifications/tasks` push surface (MAY, declined) | **`D-114-X`** — new; unowned; exposure assessed (pmcp advertises no `taskIds`, so a suite has nothing to grade) |
| the four still-deferred CONTEXT.md items | **`D-114-Y`** — new |
| inherited D-113-Q/R/S/T/U/V/W | § *Inherited from Phase 113* — new |

**`D-114-Y` restates the four CONTEXT.md deferrals as considered-and-declined**, not missed: (1) the
broader server-directed-handle client-compatibility question, cross-referenced to `D-114-K` rather
than duplicated; (2) the configurable proxy-header / claim-based identity source — the named future
closure for TASK-05's scope gap; (3) per-tool configurability of the `tasks/update` transition,
carrying 114-17's trap that **`tasks/update` leaves a fully-answered task at `working` and nothing in
the SDK promotes it to `completed`** (`s50::run_worker` is the reference shape; a plan assuming
otherwise ships a demo that hangs); and (4) **UNAS-01 — explicitly NOT folded in.** Recorded so that
*"Phase 114 touched `Mcp-Name`, did it quietly take UNAS-01 too?"* has a written answer: **it did
not.**

**Frontmatter validation: 20/20 `114-*-PLAN.md` files valid.**

> **A thirteenth measured plan-text defect.** The plan's criterion names
> `gsd-sdk query frontmatter.validate`. That form errors *"file and schema required"* for every file
> — the dot-subcommand dispatcher does not forward the positional schema argument. The working
> invocation is `gsd-sdk query frontmatter validate <file> --schema plan`. Measured across all 20:
> **valid, 0 invalid.**

**Commit:** `e7b25072`

---

## Task 4 — phase sign-off checkpoint: **APPROVED**

**Type:** `checkpoint:human-verify gate="blocking"`

| field | value |
|---|---|
| **Response** | **approved** — no changes requested |
| **Approved by** | **Guy Ernest (owner)** |
| **Date (UTC)** | **2026-08-01** |
| **Mechanism** | Answered in reply to the structured checkpoint this plan returned to the orchestrator. **A genuine human answer to a blocking gate, NOT an auto-approval** — auto-advance was off (`workflow._auto_chain_active: false`, no `workflow.auto_advance` key), and the executor did not self-approve. |

### The approval was INFORMED, and that is recorded rather than assumed

**The checkpoint material was presented in full before the answer**, including the three things a
nominal approval would have skipped past:

1. **The gate figures**, both green and red — `make quality-gate` exit 0 at 4899/294, and **both red
   gates**: `make test-feature-flags` (49 errors at the phase base → 62 at HEAD) with the **+13 delta
   attributed symbol by symbol** in `D-114-U`, and `make doc-check` (26 errors, byte-identical at
   both commits) in `D-114-V`.
2. **The `[~]` bookkeeping with `## Verdict` still `PENDING`** — i.e. that this approval closes the
   sign-off and **nothing else**.
3. **`D-114-P`** — the measured conformance gap where three `TaskRouter` fall-through legs answer
   `-32603` on a v2 `tasks/get` for which the extension makes `-32602` a **MUST**, leaving a
   router-backed v2 deployment non-conformant. Booked to Phase 118, **not** closed here.

### Both corrections to the checkpoint script were SURFACED and ACCEPTED

The plan's own Task 4 text asked the reviewer to confirm two statements that measurement had already
falsified. Both were relayed **before** the answer and accepted as restated:

1. **DQ7's parenthetical is WRONG.** The plan asks for confirmation of *"no contract YAML —
   `../provable-contracts/` is absent and `make comply` is repo-local and informational"*. The
   accepted statement is: **an owner waiver (option-b, Guy Ernest, 2026-07-28) resting SOLELY on D-18
   provisional values**, with `contracts/` **in-repo, git-tracked and graded** — re-measured this run
   (`make comply` exit 0; CB-1200 finds 2 contract files, CB-1202 2/2 keywords, CB-1205 provability
   invariant satisfied, CB-1305 2/2 classified). The *"nowhere to write it"* premise was falsified by
   `114-CONTRACT-DECISION.md` §1.5 **before** the owner decided, and may not be cited as precedent.
2. **DQ6's condition is now a ONE-repository check** on `modelcontextprotocol/ext-tasks` only. The
   core spec published `schema/2026-07-28/`; the extension has not.

### What this approval does NOT do

**It closes the sign-off checkpoint and nothing else.** Stated explicitly because a phase whose plans
have all shipped invites the assumption that it is finished:

- **TASK-01…TASK-06 stay `[~]`.** No checkbox flips. They flip as a group, only on a
  `PUBLISHED-CONFIRMED` landing.
- **`114-SPEC-RECHECK.md` `## Verdict` stays `PENDING`.** The 2026-08-01 run landed `STILL-ABSENT`
  (partial publication) and that is unchanged by a human answering a different question.
- **The D-18 publication hold is UNTOUCHED.** `ext-tasks` still carries `draft/` only, 0 tags, 0
  releases, unchanged since 2026-05-22.
- **Phase 114 stays `[~]` in `ROADMAP.md` and `completed_phases` stays `59` in `STATE.md`.** All 20
  plans have shipped, but **the phase is not complete while the hold is engaged** — the same marker
  and the same reasoning Phase 113 carries for its own publication block.
- **The carried contract-first waiver is NOT discharged.** Its condition is the same DQ6 condition,
  which is unmet.

> **A derived-view disagreement that is EXPECTED and must not be "fixed".**
> `gsd-sdk query state.json` **recomputes** `completed_phases` from `ROADMAP.md` and reports **60**,
> while `STATE.md` correctly **stores 59**. The stored value is the authoritative one. During this
> plan the SDK helpers twice tried to mark Phase 114 `[x]` and bump the counter — because every plan
> slot now has a SUMMARY — and both were reverted. **A future reader seeing the derived 60 should not
> edit `STATE.md` to match it.**

### Deferred and open items are NOT closed by this approval

`D-114-P` (router `-32603` vs the `-32602` MUST), `D-114-M` and `D-114-T` (the sibling `TaskRouter`
questions), `D-114-Q`, `D-114-S` (nothing watches `ext-tasks`), `D-114-U`, `D-114-V`, `D-114-W`,
`D-114-X`, `D-114-Y`, and the seven inherited **D-113-Q/R/S/T/U/V/W** all remain open exactly as
`deferred-items.md` records them. **`D-113-U` still needs an owner before this branch merges.**

**Commit:** `Task 4 record — see § Self-Check`

---

## Decisions made

**1. `resultType:"task"` is CONFORMANT-BY-EXTENSION, not prospective DRIFT.** The amendment required
this be decided **explicitly, not absorbed**. Measured against the **published** core
`schema/2026-07-28/schema.ts` (98 426 bytes, fetched via `gh api`): `Result.resultType` is
**required** (*"Servers implementing this protocol version MUST include this field"*, `:224-234`) and
`export type ResultType = "complete" | "input_required" | string;` (`:216`). `"task"` is not a named
upstream value; it is admissible **through the union's open `| string` tail**, and the
`io.modelcontextprotocol/tasks` extension is precisely what names it (vendored `schema.ts:228-229`,
*"The resultType field MUST be set to `"task"`"*). **An extension supplying a value through a
deliberately open union is the mechanism working as designed.** Rows 16-17 nevertheless stay held,
because the mandating sentence lives in the unpublished draft.

**2. Phase 112's absent-`resultType`-means-`complete` decoding is the CONTRACT, not a tolerance —
the 2026-07-29 advance observation is WITHDRAWN.** That run wrote *"a tolerance, not the contract, if
upstream requires the field."* The published core states the opposite, verbatim: *"For backward
compatibility, when a client receives a result from a server implementing an earlier protocol version
(which does not include `resultType`), the client **MUST** treat the absent field as `"complete"`."*
pmcp's decoding — and 114-19's named client arm — **are** the contract.

**3. The `-32003` vs `-32021` disagreement PERSISTS, so DQ3's split stands.** Published core declares
`MISSING_REQUIRED_CLIENT_CAPABILITY = -32021` (`:442`) beside `HEADER_MISMATCH = -32020` and
`UNSUPPORTED_PROTOCOL_VERSION = -32022`; **`-32003` is absent from the published core codes, which is
the EXPECTED result and CONFIRMS DQ3** — row 31 sources it to pmcp's own `AUTHENTICATION_REQUIRED`,
never to core. The ext-tasks prose saying `-32003` is still unpublished `draft`, unchanged since
2026-05-22, so there is no *published* extension prose to have been corrected. Keep both codes, two
meanings.

**4. DQ7 is CITED, not decided (T-114-106).** The waiver is the owner's. This plan read the record,
confirmed its follow-up obligation row exists, and re-measured its residual costs. It did **not**
choose option-b on its own authority, and the corrected framing (see Task 2) is what goes to the
owner at sign-off.

**5. Ledger IDs resolved by redirect, not by rewriting history.**

---

## Deviations from Plan

### Auto-fixed issues

**1. [Rule 1 — Bug] Two broken intra-doc links introduced by 114-19**
- **Found during:** Task 1, by comparing `cargo doc` warnings against the base-commit manifest
- **Issue:** `[`Error::Parse`]` (`src/client/mod.rs:1435`) and `[`Error::Capability`]` (`:2023`) name
  variants that do not exist on `Error`. HEAD carried 30 doc warnings against base's 28.
- **Fix:** `[`Error::Protocol`]` carrying `ErrorCode::PARSE_ERROR` (what `Error::parse` builds) and
  `[`Error::UnsupportedCapability`]` (what `Error::capability` builds). Warning count back to 28 with
  a byte-identical per-file distribution.
- **Files:** `src/client/mod.rs` — **Commit:** `6be9f5fe`

### Scope taken beyond `files_modified`, deliberately

The plan's frontmatter lists `src/server/task_dispatch.rs` and `crates/pmcp-tasks/src/constants.rs`.
Task 1's `<action>` orders a tree-wide sweep and its `<acceptance_criteria>` require changes in
`src/types/tasks.rs`, so the narrower list is inconsistent with the task it describes. **Seven files
were changed, all doc-comment-only**, and `src/server/task_dispatch.rs` was **verified and left
untouched** — both its known-false rustdocs were already corrected by 114-05/114-08, which is what
the task asked to be checked rather than assumed.

### Criteria measured as unsatisfiable (recorded, not worked around)

| criterion | measurement |
|---|---|
| *"No `## BLOCKING: TASK-05 security defect` heading exists"* | heading EXISTS with a **NONE FOUND** body — tests existence where it must test content |
| *"`grep -rn "advertises no" src/server/task_dispatch.rs` returns nothing"* | **3 hits, all TRUE**, about a *backendless server* |
| *"`make test-feature-flags` exits 0 for all four rows"* | red at the phase base (49 errors) and at HEAD (62) — D-114-E / D-114-U |
| *"`cargo semver-checks` … 223/223"* | true against the **phase base**; 222/223 against **published 2.17.0**, identically at both commits — D-114-W |
| *"`gsd-sdk query frontmatter.validate`"* | errors for every file; needs `frontmatter validate <file> --schema plan` |
| DQ7 framed as *"`../provable-contracts/` is absent … so no contract YAML"* | premise falsified by `114-CONTRACT-DECISION.md` §1.5 **before** the owner decided; option-b rests solely on D-18 provisional values |

**That is six, bringing Phase 114's measured plan-text defect count to thirteen.**

### Not fixed, filed instead

**D-114-U** (+13 `test-feature-flags` dead-code sites), **D-114-V** (`doc-check` red and outside
`quality-gate`), **D-114-W** (the two semver baselines + the pmat/STATE.md correction). Each names an
owner or states it is unowned.

---

## Known Stubs

None. This plan changed no behaviour: `git diff 6be9f5fe..HEAD -- src/ crates/ Cargo.toml Cargo.lock`
is byte-EMPTY, and Task 1's own diff carries **zero** non-comment lines.

---

## Threat Flags

None. No new network endpoint, auth path, file-access pattern or schema change at a trust boundary.
`T-114-96` and `T-114-SC` are both closed by measurement: the `Cargo.toml`/`Cargo.lock`/
`crates/pmcp-tasks/Cargo.toml` diff is EMPTY both in the working tree and across the whole phase.

---

## For the human reviewer at Task 4 — **ANSWERED 2026-08-01: approved**

> **This section is retained as written, unedited, because it is the material the reviewer was shown
> before answering.** Rewriting it after the fact would erase the evidence that the approval was
> informed. The outcome is recorded in § *Task 4*.

The sign-off checkpoint is **NOT self-approved**. Everything it asks you to confirm is measured above:
the gate numbers in § *Task 2*, the bookkeeping in § *Task 3*, and the delivered DQ1/DQ4 outcomes in
the example transcript (§ *Task 2* item 9, demos [2] and [4]).

**Two corrections to the checkpoint script itself, so you are not asked to confirm something false:**

1. **DQ7's parenthetical in the plan is wrong.** It says *"no contract YAML — `../provable-contracts/`
   is absent and `make comply` is repo-local and informational"*. The correct statement is: **an
   owner waiver (option-b, yours, 2026-07-28) resting SOLELY on D-18 provisional values.** `contracts/`
   **is** in-repo and **is** graded — that premise was measured false and withdrawn *before* you
   decided.
2. **DQ6's condition is now a ONE-repository check.** The core spec published on 2026-07-28; only
   `ext-tasks` is outstanding.

---

## Self-Check: PASSED

All 12 claimed files exist on disk. All 6 claimed commits resolve in `git log`:

| commit | task |
|---|---|
| `6be9f5fe` | Task 1 — era-qualify every tasks doc this phase falsified |
| `9b7d9a01` | Task 2 — record the three findings the whole-phase gate run measured |
| `e7b25072` | Task 3 — book TASK-01..06, finalize the hold record, sweep the ledger |
| `64ec87e5` | this SUMMARY |
| `de1f4622` | plan metadata; Task 4 returned unanswered for human sign-off |
| `cb0d2ecc` | **Task 4 — the sign-off approval record** |

No claimed artifact is missing.

### Hold invariants re-asserted AFTER the approval

Re-measured on disk after `cb0d2ecc`, because *"the human approved"* is exactly the moment a hold gets
released by accident:

| invariant | measured | required |
|---|---|---|
| `grep -cE '^- \[~\] \*\*TASK-0[1-6]\*\*' .planning/REQUIREMENTS.md` | **6** | 6 |
| `grep -cE '^- \[x\] \*\*TASK-0[1-6]\*\*' .planning/REQUIREMENTS.md` | **0** | 0 |
| `114-SPEC-RECHECK.md` `## Verdict` | **`PENDING`** | `PENDING` |
| `grep -c '^- \[~\] \*\*Phase 114' .planning/ROADMAP.md` | **1** | 1 |
| `STATE.md` `completed_phases` | **59** | 59 |
| `git diff --diff-filter=D HEAD~1 HEAD` | **no deletions** | none |

**The approval closed the checkpoint. It moved no checkbox, no verdict and no counter.**
