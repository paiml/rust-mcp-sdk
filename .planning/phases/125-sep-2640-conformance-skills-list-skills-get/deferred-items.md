# Deferred items — phase 125

Out-of-scope discoveries logged during execution. Per the executor SCOPE
BOUNDARY rule these are NOT fixed by this phase: they are pre-existing
failures in surfaces this phase's changes did not cause.

## `make book-test` is red repo-wide (found during 125-04 Task 3)

**Status:** pre-existing, MEASURED against HEAD, not caused by phase 125.

`make book-test` (`cd pmcp-book && mdbook test`) exits 2. `mdbook test` is
not linking the `pmcp` rlib, so every doctest that opens with
`use pmcp::...` fails to compile with:

```
error[E0433]: cannot find module or crate `pmcp` in this scope
    = help: you might be missing a crate named `pmcp`
```

**Measured blast radius (2026-09-02):** 27 `test result: FAILED` lines and
324 occurrences of that error, across 26 chapter files — `introduction.md`,
`ch01`…`ch15`, `ch23`, `ch27`, and the `ch12-*` family. It is not specific
to any chapter's content.

**Baseline proof.** The HEAD version of `pmcp-book/src/ch12-8-skills.md` was
restored, `make book-test` re-run, and the working-tree version restored
afterwards (byte-identical, verified by `diff`). Baseline and post-change
runs agree exactly:

| Measurement | HEAD baseline | After 125-04 Task 3 |
|---|---|---|
| exit code | 2 | 2 |
| `test result: FAILED` lines | 27 | 27 |
| `unlinked crate pmcp` errors | 324 | 324 |
| ch12-8 failing doctests | 2 (lines 5, 360) | 2 (lines 5, 412) |

The ch12-8 line numbers move only because Task 3 inserted prose above the
closing doctest; it is the same two blocks.

**Why it is not fixed here.** It is a book-harness/toolchain problem
(mdbook's `--library-path` / `extern` wiring), not a documentation-content
problem, and fixing it touches build tooling no plan in phase 125 owns.
`make book-test` is deliberately NOT chained into `make quality-gate`
(125-04-PLAN Task 3 `<read_first>` records this), so it gates neither the
pre-commit hook nor CI, and the red is invisible to both.

**What DOES cover the changed chapter content.** The book chapter's closing
doctest is a byte-equal mirror of the `src/server/skills.rs` module
doctest, which `cargo test --doc --all-features` compiles (479 passed). And
because both are `rust,no_run` — compiled but never EXECUTED by either
harness — 125-04 added
`src/server/skills.rs::the_module_doctest_assertions_actually_hold`, a real
unit test that runs the snippet's assertions and additionally proves the
snippet produces a conforming `skills/list` entry.

**Suggested owner:** a docs/tooling phase that repairs the mdbook link path.
Until then, treat `make book-test` as informational.

## `fuzz.yml`'s two SECONDARY jobs still enumerate only four targets (found during 125-05 Task 2)

**Status:** pre-existing shape, widened in scope by this plan, deliberately
not changed.

125-05 added `fuzz_skill_entry` to the `fuzz` job's `strategy.matrix.target`
list, which is what makes the target run on the daily schedule and on every
PR touching `src/**` or `fuzz/**`. That matrix is the one
`tests/skills_routing.rs::fuzz_skill_entry_is_registered_and_scheduled`
asserts against.

Two SIBLING jobs in the same file carry their own hardcoded shell lists that
were NOT extended:

| Job | Line (2026-09-02) | Trigger |
|---|---|---|
| `fuzz-coverage` | the `for target in protocol_parsing jsonrpc_handling transport_layer auth_flows` loops (corpus seeding + `cargo fuzz coverage`) | schedule / manual |
| `fuzz-24h` | the same four-name loop, 6 hours each | manual only |

**Why it is not fixed here.** The plan's requirement (R-34) is that the
target be EXECUTED on a schedule, and the `fuzz` job's matrix delivers that.
The `fuzz-24h` job is `workflow_dispatch`-only and its 6-hours-per-target
budget already totals 24 h across four names; adding a fifth changes that
job's runtime contract, which is a CI-budget decision this plan does not
own. `fuzz-coverage` is a reporting job, not a gate.

**A real consequence worth stating:** coverage reports for `fuzz_skill_entry`
will not be produced until someone extends those loops, so the target is
fuzzed but not coverage-measured.

**Suggested owner:** a CI phase that converts the two hardcoded loops to read
the same matrix list, so a future target enrols in all three jobs at once —
the failure mode here is the same "an explicit list does not grow" shape that
`scripts/lint-plan-verify-commands.sh` documents at length for
`LINTED_PHASES`.

## Both `make lint` and `make lint-plans` were RED at HEAD (found during 125-05 Task 3)

**Status:** FIXED by 125-05, recorded here because the finding is about the
phase's process rather than about the code.

Both are `quality-gate` legs, both were red before 125-05's Task 3, and both
were introduced by earlier plans of THIS phase:

| Leg | Defect | Introduced by | Why nothing caught it |
|---|---|---|---|
| `make lint` | `build_skills_list_response` took an owned `Vec<Value>`, tripping pedantic `clippy::needless_pass_by_value` | 125-02 | `make lint` adds `-W clippy::pedantic -W clippy::nursery`; the bare `cargo clippy --all-targets --all-features -- -D warnings` that 125-01..04 each ran is strictly weaker, as CLAUDE.md states outright |
| `make lint-plans` | 12 D-19 violations — a build/test invocation piped with no `pipefail`, so a FAILING build reports PASS — across all five `125-*-PLAN.md` files | plan authoring | The lint runs only inside `make quality-gate`, and no plan of this phase ran it |

Both were MEASURED against HEAD before being fixed (`src/server/skills.rs`
restored to HEAD, `make lint` re-run, identical error; no `*-PLAN.md` file
changed since plan creation). Recorded because the pattern — a phase running
narrower commands than its own gate for four consecutive plans — is exactly
what 125-05 exists to make impossible, and it went unnoticed one gate leg
over from where the plan was looking.

## Three 125-REVIEW.md findings carried forward unfixed (UAT decision, 2026-09-02)

**Status:** OPEN. Human decision recorded in `125-UAT.md` test 3 — deliberately
carried forward, NOT silently dropped.

Ten findings (2 CRITICAL + 8 WARNING) were raised by `/code-review max` over the
phase diff. Re-measured against HEAD `1b10e3fb` at UAT time: **6 fixed, 1
accepted, 3 open.** Fixed by `6af1c120` + `1b10e3fb`: CR-02, WR-01, WR-02,
WR-06, WR-07, WR-08. Accepted as residual risk: CR-01 (see `125-UAT.md` test 1).
The remaining three:

| Finding | Site | Why it is still open |
|---|---|---|
| WR-03 | `src/server/builder.rs:1501` | `finalize_skills_resources` panics inside a `Result`-returning `build()`. A registry that built before Phase 125 — frontmatter `name` disagreeing with the final URI segment — now aborts the process instead of returning `Err`. `ServerBuilder::skills` is `#[must_use]` and infallible, so the documented happy path cannot handle it; `try_skills()` is the only escape hatch, and four rustdoc sites do not say so. |
| WR-04 (partial) | `src/server/core.rs:2563` | The control-character neutralization landed in `1b10e3fb`, closing the log-injection half. The COVERAGE half did not: `truncated_uri_for_error` has zero direct tests — the whole tree holds two references, its definition (`:2563`) and one call site (`:2663`). Neither the truncation branch, the `…` marker, nor the multibyte-safety property its rustdoc argues for is ever executed. |
| WR-05 | `src/server/streamable_http_server.rs:4149,4280` | `assemble_skills_list_with_middleware` / `assemble_skills_get_with_middleware` have no test coverage. Four references exist, all inside that one file (defs 4149/4280, calls 5582/5600). Every skills wire test spawns via `spawn_default_config`, which installs no HTTP middleware, so `handle_post_request` always takes the fast path. The file's own rustdoc says both paths "MUST exist or the two POST paths diverge on which servers can answer the method at all" — which is exactly the divergence nothing would catch. |

**Of the three, WR-03 is the one with user-visible reach** — it is a behavior
regression for an existing registry shape, not a test-coverage gap. The other
two are absent guards against future divergence.

**Suggested owner:** the next phase of the v2.7 milestone. WR-03 is a small,
self-contained fix (return `Err` from `finalize_skills_resources`, or document
the panic at the four rustdoc sites); WR-04 and WR-05 are test-only additions.
