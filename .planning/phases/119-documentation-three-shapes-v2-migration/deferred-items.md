# Deferred Items — Phase 119

Out-of-scope discoveries logged during execution, plus the measured baselines a gate change
lands against. NOT fixed by the plan that found them.

## From plan 119-03 (the D-14 baseline, taken BEFORE the D-13 gate change)

**The claim this section exists to support: at the moment `make test-examples` was made strict,
every example the repaired gate can reach already built clean — so the first red that gate ever
shows belongs to whoever caused it, not to Phase 119.**

**Measurement base.** Commit `aa0e6c9a279dde94435567b3ae9c8663de5c71d3` (`aa0e6c9a`), measured
2026-08-19 in the phase-119 wave-3 executor worktree
(`.claude/worktrees/agent-ab94b1e3052772ea2`, branch `worktree-agent-ab94b1e3052772ea2`, forked
from `gsd/phase-119-...` after plans 119-01 and 119-02 landed). Re-measured here rather than
copied from `119-RESEARCH.md` § F-5: research measured on 2026-08-18 and the tree has moved since.

### Measurement A — the three trees the repaired gate DOES cover

Full build output captured per tree; `error`- and `warning`-prefixed lines counted in each log.

| Tree | Command | Example targets | Exit | `^error` lines | `^warning` lines |
|---|---|---|---|---|---|
| root (`pmcp`) | `cargo build --all-features --examples` | **85** (81 declared `[[example]]` + 4 auto-discovered) | **0** | **0** | **0** |
| `crates/pmcp-agent/examples/` | `cargo build -p pmcp-agent --all-features --examples` | **1** (`s50_standalone_vs_sampled.rs`) | **0** | **0** | **0** |
| `crates/pmcp-team-servers/examples/` | `cargo build -p pmcp-team-servers --all-features --examples` | **1** (`doc_review_team.rs`) | **0** | **0** | **0** |

Target counts are from `cargo metadata --no-deps` (kind `example`), not from `ls`, so a declared
example whose file moved would still be counted.

**Confirms research § F-5 exactly: 87 example targets, zero errors, zero warnings.** The strict
gate therefore lands against a clean baseline. There is no triage backlog being adopted, and no
pre-existing failure that could be mistaken for a Phase 119 regression.

### Measurement B — the example sub-crates the repaired gate does NOT cover

Each directory under `examples/` that carries its own `Cargo.toml`, built with
`cargo build --manifest-path examples/<dir>/Cargo.toml` (wasm crates additionally with
`--target wasm32-unknown-unknown`, which is installed locally).

| Sub-crate | Workspace status | Command | Exit | `^error` lines | Classification |
|---|---|---|---|---|---|
| `examples/25-oauth-basic` | **workspace MEMBER** (`Cargo.toml:801` members list) | native | **0** | 0 | **clean** |
| `examples/test-basic` | **workspace MEMBER** | native | **0** | 0 | **clean** |
| `examples/wasm-mcp-server` | standalone (declares its own `[workspace]` table) | `--target wasm32-unknown-unknown` | 101 | 5 (4 real + 1 summary) | **Pre-existing API drift, NOT a code defect of this phase.** `2× E0425` (`pmcp::types::InitializeParams`, `pmcp::types::CallToolParams` no longer exist), `1× E0422` (`pmcp::types::ListToolsParams`), `1× E0639` (`#[non_exhaustive]` struct literal). It takes `pmcp = { path = "../..", default-features = false, features = ["wasm"] }` (`:11`), so these are real drift against the current SDK — measured under the wasm target, which is the crate's actual target, per the plan's requirement not to condemn a wasm crate on a native build |
| `examples/26-server-tester` | workspace-EXCLUDED (`Cargo.toml:801`) | native | 101 | 1 | **UNMEASURABLE FROM A NESTED WORKTREE — see caveat below.** Last faithful measurement is `118.1-.../deferred-items.md` § *From plan 118.1-03*, taken at base commit `2ab06a44`: **8 pre-existing errors** in three classes — `1× E0432` + `1× E0433` (`pmcp::client::auth` is behind the `http-client` feature while the crate's `Cargo.toml:17` asks only for `streamable-http`), `3× E0599` (`reqwest::ClientBuilder::tls_danger_accept_invalid_certs` removed in reqwest 0.13), `3× E0639` (`ClientCapabilities` / `CallToolResult` are `#[non_exhaustive]` and are still built with struct literals). Recorded here as a CITED PRIOR, not a re-measurement |
| `examples/27-course-server-minimal` | workspace-EXCLUDED | native | 101 | 1 | **UNMEASURABLE FROM A NESTED WORKTREE.** Research measured it clean (exit 0) from the main checkout on 2026-08-18 |
| `examples/mcp-apps-chess` | workspace-EXCLUDED | native | 101 | 1 | **UNMEASURABLE FROM A NESTED WORKTREE.** Research: clean from the main checkout |
| `examples/mcp-apps-map` | workspace-EXCLUDED | native | 101 | 1 | **UNMEASURABLE FROM A NESTED WORKTREE.** Research: clean from the main checkout |
| `examples/mcp-apps-dataviz` | workspace-EXCLUDED | native | 101 | 1 | **UNMEASURABLE FROM A NESTED WORKTREE.** Research: clean from the main checkout |
| `examples/wasm-client` | workspace-EXCLUDED | `--target wasm32-unknown-unknown` | 101 | 1 | **UNCLASSIFIED — unmeasurable from a nested worktree.** The wasm-target build was attempted precisely so this crate would not be recorded as "broken" on the strength of a native build (research assumption A3), but the workspace-nesting artifact fires before compilation begins, so the target makes no difference from here. Recorded as UNCLASSIFIED with the reason stated — explicitly NOT as broken |
| `examples/wasm` | neither a member, nor excluded, nor standalone | native | 101 | 1 | **NOT a code defect** — `error: current package believes it's in a workspace when it's not`. A manifest/workspace-membership artifact that reproduces in the main checkout too (research § F-5 recorded the same single error there). Fixing it means one of: adding it to `members`, adding it to `exclude`, or giving it an empty `[workspace]` table |

**The nested-worktree caveat, and why six rows above say UNMEASURABLE.** This measurement was taken
inside a git worktree that lives at `.claude/worktrees/agent-<id>/` — physically *underneath* the
main checkout. For a sub-crate listed in the worktree root's `[workspace] exclude`, cargo declines
that root and keeps walking **upward**, where it finds the main checkout's
`/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml`. That manifest's `exclude` paths are
relative to *its* root, so they do not match `.claude/worktrees/agent-<id>/examples/<dir>`, and
cargo aborts with `current package believes it's in a workspace when it's not` before compiling a
single line. The failure is therefore an artifact of *where the measurement was taken*, not a
property of the crate — which is exactly why each affected row is recorded as UNMEASURABLE rather
than as one error. Two rows escape it: the two workspace MEMBERS (cargo resolves them against the
worktree root and they build) and `wasm-mcp-server` (its own `[workspace]` table stops the upward
walk). A maintainer wanting the faithful numbers should re-run, **from the main checkout**:

```
cargo build --manifest-path examples/26-server-tester/Cargo.toml
cargo build --manifest-path examples/wasm-client/Cargo.toml --target wasm32-unknown-unknown
```

**A correction to research § F-5's framing:** it grouped `examples/25-oauth-basic` and
`examples/test-basic` under "workspace-EXCLUDED example sub-crates". They are not excluded — both
are declared workspace **members** (`Cargo.toml:801`), so `cargo build --workspace` already gates
them. Their build results are unchanged (clean); only the label was wrong.

### Why the excluded sub-crates are DELIBERATELY outside the repaired gate

`scripts/run-example-builds.sh` builds the three trees in Measurement A and **no**
`examples/<dir>/Cargo.toml` sub-crate. Widening it would import at least 8 known-pre-existing
errors (`26-server-tester`), 4 more pre-existing wasm-target errors (`wasm-mcp-server`), one
manifest artifact (`wasm`), and one crate that cannot even be classified from a worktree
(`wasm-client`) — into a **documentation** phase whose gate change exists to remove a false green,
not to adopt a repair project. Neither `cargo build --workspace` nor `make lint` has ever gated
these crates either (as `118.1-.../deferred-items.md` records for `26-server-tester`), so keeping
them out preserves the status quo rather than creating a new hole.

**Owner for the residual items: whoever next owns the standalone example crates.** The reqwest,
feature-flag and `#[non_exhaustive]` halves of `26-server-tester` are independent of each other
and of documentation work; `wasm-mcp-server`'s four errors are a straight rename-follow against
the current `pmcp::types` surface; `examples/wasm`'s error is a one-line manifest decision.

### The defect research surfaced but D-13 did not name

**The pre-repair `test-examples` loop iterated `ls examples/*.rs` and nothing else.** It therefore
never reached `crates/pmcp-agent/examples/` or `crates/pmcp-team-servers/examples/` — meaning
**two of D-15's six gated examples (`s50_standalone_vs_sampled`, `doc_review_team`) were outside
the gate's reach entirely**, independent of the swallow bug. Even a loop that failed correctly
would have been blind to them. Task 2 widens the gate to all three trees, and Task 3's second
negative control exists specifically to prove that widening is real rather than asserted.

### Also discovered, NOT addressed here

- **Eleven other workspace members carry example targets that the repaired gate still does not
  reach.** From `cargo metadata --no-deps`: `cargo-pmcp` (14), `pmcp-server-toolkit` (5),
  `pmcp-openapi-server` (3), `mcp-tester` (2), `pmcp-workbook-compiler` (2), and one each in
  `pmcp-cfn-renderer`, `pmcp-toolkit-postgres`, `pmcp-toolkit-mysql`, `pmcp-toolkit-athena`,
  `pmcp-sql-server`, `pmcp-workbook-server` — **32 example targets in total**. The repaired gate
  covers the three trees the phase's D-13/D-15 scope names (87 targets) and deliberately stops
  there; a `--workspace --examples` widening is a larger blast radius than a documentation phase
  should take, and it was not measured here. Owner: whoever next revisits the example gate's
  scope. Note the widening is cheap to attempt — these crates are workspace members, so no
  `--manifest-path` and no nesting artifact is involved.
- **`119-VALIDATION.md` names this record `119-03-BASELINE.md`.** The plan's `files_modified`,
  `<artifacts_this_plan_produces>` and every acceptance criterion say
  `deferred-items.md`; the validation map's 119-03 · T1 row says
  `test -f .planning/phases/119-*/119-03-BASELINE.md`. The plan is authoritative and this file
  follows it, so that one validation row will not match on disk. Not corrected here because
  `119-VALIDATION.md` is outside this plan's file scope and is written by sibling plans in the
  same wave. Owner: plan 119-10 (the closing gate) or the orchestrator at merge — the fix is to
  point that row at `deferred-items.md`.


---

## DEFERRED (119-10): two new test files SHIP while depending on example binaries a published-crate `cargo test` can never produce

Plan 119-10's closing gate was required to review the packaging disposition of all three test
files this phase added and confirm each is a **deliberate decision rather than an accident**.
One is deliberate; two are accidents, and they are recorded here rather than fixed.

**Measured, not inferred.** This worktree started with an empty `target/`, which reproduced the
downstream shape exactly. `cargo test --test docs04_examples_run` failed **3 of 3** legs with
`target/debug/examples/<name> is missing. This leg FAILS rather than skipping, by design`
(`tests/common/example_process.rs:104`). The legs only went green after the three example
binaries were built by hand.

| File | `cargo package --list` | Deliberate? |
|---|---|---|
| `tests/windows_disclosure_tripwire.rs` | **excluded** | **Yes** — decided and reasoned in 119-10 Task 1, with a `# Why:` block in `Cargo.toml` |
| `tests/docs04_examples_run.rs` | ships | **No** — no plan recorded a disposition |
| `tests/docs06_v2_examples_run.rs` | ships | **No** — no plan recorded a disposition |

**Why the two that ship are a hazard.** Neither can pass on the published crate:

- `docs04_examples_run` runs `s50_standalone_vs_sampled` (`crates/pmcp-agent/examples/`) and
  `doc_review_team` (`crates/pmcp-team-servers/examples/`). Those crates are not part of the
  published `pmcp` crate, so those binaries can never exist downstream.
- `docs06_v2_examples_run` runs `s47_v2_stateless_mrtr`, `s48_v2_mrtr_client` and
  `s53_v2_agent_client`. All three declare `required-features` absent from
  `default = ["logging", "v1-compat"]` (`Cargo.toml:711,716,760`), so a downstream
  `cargo test` never builds them. `s53` additionally needs the **path-only** `pmcp-agent`
  dev-dependency (`Cargo.toml:251`), which is stripped on publish.

The missing-binary path **panics**; it does not skip. That is deliberate and correct in-repo —
a skip would restore the unenforced criterion these legs exist to close — but it is the exact
failure `Cargo.toml`'s own exclude commentary warns about: *"shipping the reader while excluding
the paths it reads would make `cargo test` panic on the published crate."*

**Why it was NOT fixed here.** This is a **pre-existing repo-wide pattern**, not something phase
119 introduced. `tests/embedded_resource_example_run.rs` and `tests/log_records_example_run.rs`
already ship today and both read `target/debug/examples/s54_v2_dual_conformance`, which likewise
declares `required-features = ["streamable-http", "testing"]` (`Cargo.toml:785`). Excluding only
the two new files would fix two of four instances and leave the convention *less* consistent than
it is now. Choosing between "every example-run test is excluded" and "the run-to-completion
helper degrades to a skip on the published crate only" is a repository-wide convention call with
a blast radius beyond a documentation phase, so it is handed forward rather than taken.

**Owner:** whoever next revisits the example-run test convention, or a release engineer who hits
it during a `cargo publish` dry run. **The fix is cheap either way** — four `exclude` entries, or
one publish-aware branch in `tests/common/example_process.rs`. **Not filed as a `WINDOWS.md`
entry on purpose:** plan 119-10's own acceptance criteria pin the ledger at `open_count: 17` /
`total_count: 23`, and appending mid-gate would have invalidated the closing measurement — the
same reasoning plan 119-05 used when it declined to append.

### Resolved by the orchestrator before this plan ran

- The `119-03-BASELINE.md` item immediately above is **CLOSED**. `119-VALIDATION.md`'s
  119-03 · T1 row now asserts `test -f "$D/deferred-items.md"` plus a `git merge-base
  --is-ancestor 5b90fdd2 9aefc939` ordering proof, exactly the fix that item requested.
