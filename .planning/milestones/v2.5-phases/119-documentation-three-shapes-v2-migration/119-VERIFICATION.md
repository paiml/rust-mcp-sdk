---
phase: 119-documentation-three-shapes-v2-migration
verified: 2026-08-19T08:55:00Z
status: passed
score: 3/3 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 119: Documentation — Three Shapes + v2 Migration Verification Report

**Phase Goal:** The milestone is documented per the house three-shapes rule (pmcp-book chapters + runnable examples + README/course), leading with the `cargo pmcp` workflow — covering both the v2.4 Agents & Teams surface (carried from Phase 111) and the v2 dual-version migration story, with runnable v2 examples verified against the shipped code.
**Verified:** 2026-08-19
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

Truths are the three ROADMAP.md Phase 119 Success Criteria, one per requirement ID. Each was
independently re-measured against the tree (not SUMMARY claims), including the three prose
judgment items the phase's own `119-VALIDATION.md` deliberately deferred to a human reader.

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | **DOCS-04** — Agents & Teams documented in three shapes (pmcp-book chapters, runnable examples, README/course), cargo-pmcp-first | ✓ VERIFIED | Book: `ch12-15-agents-as-mcp-clients.md` (224 lines) + re-parented `ch17-04-sampling-hosting.md` + `ch12-16-agent-teams.md` (267 lines), all reachable from `pmcp-book/src/SUMMARY.md`, `mdbook build` exit 0. Examples: `cargo test --features full --test docs04_examples_run` → **3 passed** (`s50_standalone_vs_sampled`, `s49_sampling_host`, `doc_review_team`, all run to completion, not just built). Course: `pmcp-course/src/part8-advanced/ch24-agents-and-teams.md` (462 lines) + `ch24-exercises.md` (211 lines), wired into `pmcp-course/src/SUMMARY.md`, `mdbook build` exit 0 (course build's own `create-missing=true` makes `test -f` the real gate — both files independently confirmed present on disk). README: `## Agents & Teams` section present at `README.md:447`. **cargo-pmcp-first, read directly, all four surfaces:** ch12-15's first `bash` block is `cargo install cargo-pmcp` / `cargo pmcp agent new research-agent` (line 84-88); ch12-16's first `bash` block is `cargo pmcp team dev` (line 90-93); ch24's first `bash` block is `cargo pmcp agent new research-agent` (line 159-163); README `## Agents & Teams`'s first block is `cargo pmcp agent new research-agent` (line 454-461). This is the manual-only item `119-10-SUMMARY.md` recorded as the sole reason it left DOCS-04 unbooked ("cargo-pmcp-first ordering, per chapter... This one gates DOCS-04") — read directly here and found satisfied on all four surfaces named. |
| 2 | **DOCS-05** — v2 migration guide + dual-version documentation: opt-in path, dual-version story, Tasks extension migration, legacy sunset policy | ✓ VERIFIED | `pmcp-book/src/ch12-17-migrating-to-mcp-2026-07-28.md` (436 lines) exists, is role-organized (`## For servers` / `## For clients` / `## For agents`), reachable from `SUMMARY.md`, `mdbook build` exit 0. Disclosure sentinel: 10 `[CONSUMER-OBSERVABLE]` markers in `.planning/WINDOWS.md`; the derived tripwire `cargo test --features full --test windows_disclosure_tripwire` → **1 passed**, no hardcoded id array (`const IDS` absent), no `include_str!` (runtime reads), citing ledger entries 12/13/19/20/23 by id in the chapter (all five found at `ch12-17-migrating…:341,351,361,373,388`). Sunset policy: `## The v1 sunset` (line 404) links `docs/v1-sunset-policy.md` twice and states no date, no competing normative claim — read directly, confirmed link-only. Tasks provisionality: `ch12-7-tasks.md:786` — "**These wire values are provisional**... Read this section as a description of what pmcp does today, not as a stable contract" — read directly, confirmed the wire-finality claim is explicitly hedged while shipped behaviors (T-1..T-7 equivalents) carry source cites into `src/types/capabilities.rs` / `src/server/task_dispatch.rs`. Transport era callouts added to `ch10-transports.md` and `ch10-03-streamable-http.md` with **zero code-block diff** (confirmed via `git show 76bb3f97` — the diff touches only prose/callout lines). CHANGELOG: `## [2.18.0] - 2026-08-16` and `## [2.17.0] - 2026-07-19` now dated (matching upstream release dates); the remaining `## [2.16.0] - Unreleased` heading is genuinely correct (verified: no `2.16.0` on crates.io sparse index, no `v2.16.0` tag on either remote — commit `235781d6`'s own message documents this investigation), so this is not a third mislabel left unfixed. |
| 3 | **DOCS-06** — Runnable v2 examples: a stateless (Lambda-style) v2 server and a v2 client/agent example | ✓ VERIFIED | `tests/docs06_v2_examples_run.rs` — `cargo test --features full --test docs06_v2_examples_run` → **1 passed** (6.89s), a real socket round trip on port 8161: `s47_v2_stateless_mrtr` (stateless server, no `initialize`, no `Mcp-Session-Id`) served against **both** `s48_v2_mrtr_client` (MRTR client) and `s53_v2_agent_client` (agent client) as live peers. `scripts/run-example-builds.sh` via `make test-examples` → **87 examples built across 3 covered trees, 0 failures**, exit 0 (re-run independently; matches the D-14 baseline in `deferred-items.md` and the orchestrator's prior measurement). No `2>/dev/null` / `\|\| true` in live script code (only in comments documenting the removed behavior). |

**Score:** 3/3 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `pmcp-book/src/ch12-15-agents-as-mcp-clients.md` | New Part III chapter | ✓ VERIFIED | 224 lines, wired into SUMMARY.md, cargo-pmcp-first |
| `pmcp-book/src/ch17-04-sampling-hosting.md` | Re-parented child chapter | ✓ VERIFIED | 103 lines, reachable as child of 12.15 |
| `pmcp-book/src/ch12-16-agent-teams.md` | New Part III chapter | ✓ VERIFIED | 267 lines, wired, cargo-pmcp-first |
| `pmcp-book/src/ch12-17-migrating-to-mcp-2026-07-28.md` | New Part III migration chapter | ✓ VERIFIED | 436 lines, role-organized, wired |
| `pmcp-course/src/part8-advanced/ch24-agents-and-teams.md` | New Part VIII chapter | ✓ VERIFIED | 462 lines, `test -f` + build both confirmed |
| `pmcp-course/src/part8-advanced/ch24-exercises.md` | Tiered exercises | ✓ VERIFIED | 211 lines, wired as SUMMARY child |
| `tests/common/example_process.rs` | `run_example_to_completion` helper | ✓ VERIFIED | 562 lines, used by docs04/docs06 legs |
| `tests/docs04_examples_run.rs` | 3 run-to-completion legs | ✓ VERIFIED | 252 lines, 3 tests enumerated + passing |
| `tests/docs06_v2_examples_run.rs` | Socket round-trip leg | ✓ VERIFIED | 251 lines, 1 test enumerated + passing |
| `tests/windows_disclosure_tripwire.rs` | Derived disclosure fence | ✓ VERIFIED | 229 lines, excluded from published crate (`Cargo.toml`), 1 test passing |
| `scripts/run-example-builds.sh` | Counted, exit-1-on-failure gate | ✓ VERIFIED | 184 lines, wired via `Makefile:277-278`, 0 `\|\| true`/`2>/dev/null` in live code |
| `.planning/phases/119-.../deferred-items.md` | Baseline + packaging findings | ✓ VERIFIED | 180 lines, D-14 baseline + D-119-10 packaging finding both present |
| README `## Protocol Versions` / `## Agents & Teams` | Index-tier sections | ✓ VERIFIED | Both present, link to book rather than restate |
| CHANGELOG.md headings | 2 mislabelled headings dated | ✓ VERIFIED | `2.18.0`→2026-08-16, `2.17.0`→2026-07-19; `2.16.0` correctly left Unreleased |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `pmcp-book/src/SUMMARY.md` | `ch12-15/16/17`, `ch17-04` | `mdbook build` (`create-missing=false`) | ✓ WIRED | exit 0 |
| `pmcp-course/src/SUMMARY.md` | `ch24-agents-and-teams.md`, `ch24-exercises.md` | `test -f` + `mdbook build` | ✓ WIRED | both files present; build exit 0 (course theme diff from `mdbook-exercises` restored post-build, per plan's documented behavior) |
| `.planning/WINDOWS.md` sentinel | `ch12-17-migrating-to-mcp-2026-07-28.md` citations | `windows_disclosure_tripwire` (runtime file reads, derived id set) | ✓ WIRED | 1 passed; entries 12/13/19/20/23 all cited |
| `Makefile:test-examples` | `scripts/run-example-builds.sh` | delegate | ✓ WIRED | `make test-examples` → 87/87, exit 0 |
| `s47_v2_stateless_mrtr` (server) | `s48_v2_mrtr_client` / `s53_v2_agent_client` (peers) | live socket, port 8161 | ✓ WIRED | `docs06_v2_examples_run` 1 passed |
| README `## Protocol Versions` | `ch12-17-migrating-to-mcp-2026-07-28.md` | link, not retell | ✓ WIRED | link present, no restated normative content |
| `ch12-17`'s sunset section | `docs/v1-sunset-policy.md` | link only | ✓ WIRED | 2 links, no restatement or contradiction (read directly) |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| docs04 legs run to completion | `cargo test --features full --test docs04_examples_run` | `3 passed; 0 failed` (0.06s) | ✓ PASS |
| docs06 v2 socket round trip | `cargo test --features full --test docs06_v2_examples_run` | `1 passed` (6.89s) | ✓ PASS |
| disclosure tripwire | `cargo test --features full --test windows_disclosure_tripwire` | `1 passed` | ✓ PASS |
| example-build gate | `make test-examples` | `87 examples built across 3 covered trees, 0 failures` | ✓ PASS |
| pmcp-book build | `cd pmcp-book && mdbook build` | exit 0 | ✓ PASS |
| pmcp-course build | `cd pmcp-course && mdbook build` | exit 0 (theme diff restored post-check) | ✓ PASS |

All example binaries were rebuilt from a clean state before each run (`cargo build --features full --example s49_sampling_host …`, `-p pmcp-agent --example s50_standalone_vs_sampled`, `-p pmcp-team-servers --features runtime --example doc_review_team`, `--features full --example s47_v2_stateless_mrtr --example s48_v2_mrtr_client --example s53_v2_agent_client`) — all compiled clean.

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Ledger State | Codebase Evidence | Verifier Verdict |
|---|---|---|---|---|---|
| DOCS-04 | 119-02, 04, 06, 07, 09 | Agents & Teams in three shapes, cargo-pmcp-first | `[ ]` Pending | All four shapes present, wired, tested green; cargo-pmcp-first read directly across all 4 new chapter/section surfaces and confirmed | **SATISFIED** — every plan correctly left `requirements-completed: []` because no single plan owned all four quarters (book/examples/README/course), and the one item `119-10` flagged as still-open (the cargo-pmcp-first manual read) is confirmed positive by this verification. Recommend the ledger owner flip DOCS-04 to `[x]`. |
| DOCS-05 | 119-01, 05, 08, 09, 10 | v2 migration guide + dual-version docs | `[x]` Complete (booked by tracking commit after wave 3) | Chapter + sentinel + tripwire + sunset link + Tasks provisionality all directly confirmed | **CONFIRMED CORRECT, evidence gap now closed.** `119-10-SUMMARY.md` flagged that "two of the three open manual-only reads... belong to DOCS-05 and were still open when it was booked." Both (sunset-policy non-restatement, Tasks provisionality) were read directly in this verification and found satisfied — the booking was directionally right even though the reads were open at the time it landed. No reversal warranted. |
| DOCS-06 | 119-03, 04 | Runnable v2 examples: stateless server + client/agent | `[ ]` Pending | Evidence complete and fully mechanized per `119-10-SUMMARY.md`; independently re-run here and green | **SATISFIED.** `119-10` explicitly recommended `/gsd-verify-work` book this `[x]`, having no manual-only verification attached. This verification concurs; no gap found. Recommend the ledger owner flip DOCS-06 to `[x]`. |

No orphaned requirements — `.planning/REQUIREMENTS.md`'s traceability table (lines 1128-1130) lists exactly DOCS-04/05/06 for Phase 119, and all three appear in at least one plan's `requirements:` frontmatter field.

### Anti-Patterns Found

None. Scanned all 16 phase-modified files (book chapters, course chapters, test files, `scripts/run-example-builds.sh`, README.md, CHANGELOG.md, the two amended ch10 chapters) for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER|placeholder|coming soon|not yet implemented` — zero hits.

### Known, Recorded (Not Phase 119 Defects)

- **Two shipping test files depend on example binaries a published-crate `cargo test` can never build** (`tests/docs04_examples_run.rs`, `tests/docs06_v2_examples_run.rs`). Confirmed genuinely recorded in `deferred-items.md` under "DEFERRED (119-10)", with the pre-existing repo-wide pattern (`tests/embedded_resource_example_run.rs`, `tests/log_records_example_run.rs`) cited as precedent. Not a phase-119-introduced regression; correctly logged rather than silently absorbed.
- **`make quality-gate`'s FUZZ leg builds nothing on stable** (pre-existing, unrelated to this phase's deliverables) — not credited toward this phase's gate evidence, consistent with the orchestrator's note.
- **`make quality-gate` does not chain either `mdbook build`** — both were run and verified independently in this verification, as `119-10`'s closing gate also did.

### Human Verification Required

None. The three items `119-VALIDATION.md` designated Manual-Only (cargo-pmcp-first ordering, sunset-policy non-restatement, Tasks provisionality) were read directly against the current tree in this verification with specific line-cited evidence recorded above, and all three resolved positively. No item remains uncertain enough to warrant deferring to a further human read.

### Gaps Summary

No gaps. All three ROADMAP Phase 119 Success Criteria (DOCS-04, DOCS-05, DOCS-06) are observably true in the codebase: artifacts exist, are substantive, are wired into their books/README/gate, and pass behavioral checks re-run independently by this verification (not inherited from SUMMARY.md claims). The requirements ledger currently reads DOCS-04/DOCS-06 as `[ ]` Pending — this verification's recommendation is that both be flipped to `[x]`, since the specific reasons `119-10-PLAN.md` cited for withholding them (an open cargo-pmcp-first read for DOCS-04, and DOCS-06 simply being outside that plan's authority to book) are resolved by this independent check.

---

_Verified: 2026-08-19_
_Verifier: Claude (gsd-verifier)_
