---
phase: 104-task-augmented-tool-results-dx
verified: 2026-07-05T03:27:36Z
status: passed
score: 7/7 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 4/7
  gaps_closed:
    - "ALWAYS requirements met and `make quality-gate` green (Success Criterion 5)"
    - "Client::wait_for_task polls tasks/get until a terminal status then returns tasks/result, honoring pollInterval (permanent hang on InputRequired, CR-01)"
    - "`make doc-check` green — book/doc links resolve (Plan 104-05's own <verification> requirement)"
  gaps_remaining: []
  regressions: []
human_verification: []
---

# Phase 104: Task-Augmented Tool Results DX Verification Report

**Phase Goal:** Close the junction between the tool contract and the tasks layer so a tool can return a task-augmented (or otherwise full) `CallToolResult` — `_meta` included — through the normal `Server` dispatch front door, instead of dispatch stringifying it into `content[0].text`. Kills the silent double-wrap bug class documented by the pmcp.run team (5 incident variants, incl. a 2-week silent production outage), and lets their three hand-rolled pre-2.11 task servers migrate onto native TaskSupport.

**Verified:** 2026-07-05
**Status:** passed
**Re-verification:** Yes — after gap closure (commits 688de942, 8983f648, ee2122b4, 137995ce, c73d6ee4, f5011384, 53807bd9)

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | (SC1/TOUT-01) A `ToolHandler` has a typed way to return a full `CallToolResult` (with `_meta`) that lands on the wire un-re-wrapped through normal `Server` dispatch; implicit sniffing rejected | ✓ VERIFIED | Unchanged from initial verification. Re-ran live: `cargo test --features full --test tool_output_passthrough` (9/9 pass, includes the new WR-03 regression test) + `--test tool_output_result_http` (1/1 pass, real StreamableHTTP round-trip) + `cargo run --example s47_task_augmented_result --features full` (exits 0, correct BEFORE/AFTER output). No regression. |
| 2 | (SC2/TOUT-02) Dispatch emits a WARN (debug-fail optional) when about to text-wrap a `Value` that structurally looks like an already-built `CallToolResult` | ✓ VERIFIED (caveat resolved) | `double_wrap_tripwire`/`looks_like_call_tool_result` (src/server/task_dispatch.rs:211-298) confirmed by direct code read: `looks_like_call_tool_result` now requires ALL object keys to be in the `CallToolResult` envelope key set (`content`/`isError`/`structuredContent`/`_meta`) before the content-array marker fires (WR-02 fix, commit ee2122b4). Chat-message payloads (`role`/`model`/`stopReason` + content array) no longer trip. Live test run: `cargo test --features full --test double_wrap_tripwire` 14/14 pass (debug, incl. new `looks_like_ignores_chat_message_payload` and `looks_like_fires_on_full_envelope_keys`) and `cargo test --release --features full --test double_wrap_tripwire` 15/15 pass (release never panics). The prior WR-02 false-positive caveat is resolved — no longer tracked as an open warning. |
| 3 | (SC3/TOUT-03) Client exposes a typed `related_task()` accessor recovering `TaskMetadata` from `_meta[related-task]` | ✓ VERIFIED | Unchanged. `cargo test --features full --test task_augmented_result` 11/11 pass (round-trip, minimal-shape, no-`_meta` cases). |
| 3b | (104-01-PLAN.md must-have) `Client::wait_for_task` polls to terminal and honors `pollInterval`/timeout, wasm-safe | ✓ VERIFIED (was FAILED) | **Gap closed.** CR-01 fixed in commit 688de942: `wait_for_task` (src/client/mod.rs:680-724) now returns `Error::Validation` immediately when `task.status == TaskStatus::InputRequired`, instead of looping forever (confirmed by direct code read of the new branch). WR-01 (sleep overshoot) fixed in commit 8983f648: the poll interval is now clamped to the remaining `max_poll_duration_secs` budget at millisecond precision before each sleep. Live regression tests re-run: `wait_for_task_surfaces_input_required_instead_of_hanging` (drives a real store task `Working -> InputRequired` server-side, asserts prompt `Error::Validation`, not a hang) and `wait_for_task_timeout_is_not_overshot_by_large_interval` (60s server interval + 1s budget reports timeout near budget) — both PASS in `cargo test --features full --test task_augmented_result` (11/11 total, including these two). Rustdoc `# Errors` section and the pmcp-book chapter updated to document the new `InputRequired` behavior. |
| 4 | (SC4/TOUT-04) A migration guide documents hand-rolled `_meta` task patterns → native `with_task_store()` | ✓ VERIFIED | Unchanged content; `docs/design/sep-1686-task-augmented-results.md` additionally corrected per WR-03 (commit 137995ce): §4 no longer claims the create-path gate "keeps precedence" on the verbatim path — it now states plainly the gate is structurally bypassed/never consulted, with the client-visible consequence spelled out and a regression test (`task_augmented_call_to_verbatim_tool_returns_plain_result`) added and passing. |
| 4b | (Plan 104-05 own `<verification>`) `make doc-check` green (book/doc links resolve) | ✓ VERIFIED (was FAILED) | **Gap closed.** Re-ran `make doc-check` live in this session: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket` — exits 0, "✓ Zero rustdoc warnings", docs generated at `target/doc/pmcp/index.html`. Fix (commit f5011384) confirmed by diff: unresolved intra-doc links for `RELATED_TASK_META_KEY`/`TaskMetadata` in src/types/tools.rs now use fully-qualified `crate::types::tasks::` paths; 8 redundant explicit link-target instances across src/types/tools.rs, src/server/typed_tool.rs, src/server/mod.rs replaced with bare `[`Type`]` references matching in-scope imports. |
| 5 | (SC5) ALWAYS requirements (unit + property + fuzz + runnable example) met AND `make quality-gate` green | ✓ VERIFIED (was FAILED) | **Gap closed.** Ran `make quality-gate` live end-to-end in this session (full log captured, ~5360 lines): `cargo fmt --all -- --check` clean → clippy (`-D clippy::all -W pedantic -W nursery -W cargo` superset, matching CI) "✓ No lint issues" (the 2 `clippy::too_long_first_doc_paragraph` errors from commit 0ba7cd88 are gone — confirmed by diff of commit f5011384 reflowing both doc comments in src/server/task_dispatch.rs into a short first sentence + detail paragraph) → build successful → 1107 lib unit tests pass + full integration suite (373 tests across ~180 files) pass → `cargo audit` "✓ No vulnerabilities found" (3 pre-existing allowed warnings; the 2 new RUSTSEC-2026-0194/0195 ignores for `quick-xml` via `umya-spreadsheet` are scoped to the unrelated workbook-compiler dependency tree, documented with rationale and a removal-tracking comment, commit 53807bd9) → zero TODO/FIXME/debt comments → zero unwrap() outside tests → ALWAYS validation (unit+property+fuzz+examples, all 44 examples incl. s47 build) → purity-check passes → log ends "✅ ALL TOYOTA WAY QUALITY CHECKS PASSED". (Fuzz-target build errors for sanitizer-coverage flags are a pre-existing environment limitation — this machine's stable toolchain lacks nightly `-Z` flags — not a phase-104 regression; the Makefile's fuzz step tolerates this and the gate still completes and reports success, exactly as in the pre-remediation initial verification run.) |

**Score:** 7/7 truths verified. All 3 gaps from the prior verification (quality-gate lint failure, wait_for_task InputRequired hang, doc-check failure) are independently confirmed closed by live command execution in this session, not by trusting SUMMARY/REVIEW-FIX claims.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/types/tasks.rs` | `TaskMetadata` struct | ✓ VERIFIED | Unchanged from initial verification. |
| `src/types/tools.rs` | `with_related_task` / `related_task` | ✓ VERIFIED | Present at :628/:658 (line numbers shifted slightly post-doc-fix, content identical); rustdoc links now fully-qualified (`crate::types::tasks::RELATED_TASK_META_KEY`, `crate::types::tasks::TaskMetadata`). |
| `src/client/mod.rs` | `wait_for_task` / `wait_for_related_task` / `WaitForTaskOptions` | ✓ VERIFIED (was VERIFIED-BUT-DEFECTIVE) | CR-01 and WR-01 fixes confirmed present by direct code read (src/client/mod.rs:680-724): explicit `InputRequired` early-return branch; budget-clamped sleep computed via `saturating_mul`/`saturating_sub` on millisecond precision. No longer defective. |
| `src/server/mod.rs` | `ToolOutput` enum + `handle_output` default + dispatch branch | ✓ VERIFIED | Unchanged functionally; doc-link cleanup only (cosmetic diff, confirmed via `git show f5011384`). |
| `src/server/task_dispatch.rs` | `resolve_tool_output`/`DispatchOutput` + `looks_like_call_tool_result`/`double_wrap_tripwire` | ✓ VERIFIED (was functionally-verified-with-lint-failure) | Doc-paragraph reflow (commit f5011384) confirmed live: `make lint` (via `make quality-gate`) reports "✓ No lint issues". WR-02 envelope-key restriction (commit ee2122b4) confirmed by code read + passing tests. |
| `src/server/core.rs` | `ServerCore` twin branch + tripwire + result_meta wiring | ✓ VERIFIED | Unchanged. |
| `tests/tool_output_passthrough.rs` | verbatim/parity/middleware-bypass tests + WR-03 regression | ✓ VERIFIED | 9/9 pass (was 8/8 — new `task_augmented_call_to_verbatim_tool_returns_plain_result` test added by WR-03 fix and passing). |
| `tests/double_wrap_tripwire.rs` | fire/no-fire/suppressed/proptest/panic tests + WR-02 precision tests | ✓ VERIFIED | 14/14 debug (was 12/12 — 2 new WR-02 precision tests), 15/15 release (was 13/13). |
| `src/server/typed_tool.rs` | `TypedToolWithResult` wrapper | ✓ VERIFIED | WR-04 resolved: `tool_with_result_and_description` sugar now exists (see below). |
| `src/server/cancellation.rs` | `set_result_meta`/`take_result_meta`/`ResultMetaHandle`/`merge_result_meta` | ✓ VERIFIED | Unchanged. |
| `tests/tool_with_result.rs` | tool_with_result wire test + WR-04 description-overload test | ✓ VERIFIED | 8/8 pass (was 7/7 — new `tool_with_result_and_description_advertises_description` test). |
| `examples/s47_task_augmented_result.rs` | BEFORE/AFTER runnable migration example | ✓ VERIFIED | `cargo run --example s47_task_augmented_result --features full` exits 0, identical correct output re-confirmed live. |
| `tests/tool_output_result_http.rs` | live HTTP `_meta`-at-top-level gate | ✓ VERIFIED | 1/1 pass, re-confirmed live. |
| `docs/design/sep-1686-task-augmented-results.md` | migration guide + bypass callout + WR-03 correction | ✓ VERIFIED | §4 rewritten per WR-03; "What IS bypassed" section confirmed present verbatim by direct read. |
| `ServerBuilder::tool_with_result_and_description` | WR-04 fix — description-setting sugar for the verbatim wrapper | ✓ VERIFIED (new) | Added in commit c73d6ee4, mirrors `tool_typed_with_description`; test confirms description lands in `tools/list` AND verbatim semantics preserved. |
| `.cargo/audit.toml` | RUSTSEC-2026-0194/0195 ignores with rationale | ✓ VERIFIED | 12-line addition (commit 53807bd9), scoped to `quick-xml` via `umya-spreadsheet` in the unrelated `pmcp-workbook-compiler` dependency tree (not this phase's code), documented rationale + removal-tracking comment. `cargo audit` (part of `make quality-gate`) confirmed passing live. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `Server::handle_call_tool` (mod.rs) | `handler.handle_output` | native dispatch call-site swap | ✓ WIRED | Unchanged, re-confirmed by passing `tool_output_passthrough`/`tool_output_result_http` tests. |
| `ServerCore::handle_call_tool` (core.rs) | `handler.handle_output` | native dispatch call-site swap | ✓ WIRED | Unchanged. |
| `Client::wait_for_task` | `Client::tasks_get` / `Client::tasks_result` | poll loop with InputRequired early-return + budget-clamped sleep | ✓ WIRED (fixed) | Confirmed by direct code read and by 2 passing live regression tests (input_required surfacing, timeout-not-overshot). |
| Payload arm (both dispatchers) | `task_dispatch::maybe_build_task_created` | create-path gate keeps precedence on Payload path; structurally bypassed on Verbatim path (now correctly documented) | ✓ WIRED (Payload only, doc corrected) | WR-03 regression test `task_augmented_call_to_verbatim_tool_returns_plain_result` passes, confirming and locking in the documented behavior. |
| `ServerBuilder::tool_with_result_and_description` | `TypedToolWithResult::new(...).with_description(...)` | new WR-04 builder overload | ✓ WIRED | Confirmed by passing `tool_with_result_and_description_advertises_description` test. |

### Data-Flow Trace (Level 4)

Not applicable in the strict UI-data sense (server/client dispatch-plumbing phase, not a rendering surface) — same as initial verification. The live HTTP wire test (`tests/tool_output_result_http.rs`) re-confirmed passing, proving `_meta` reaches raw wire bytes verbatim.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| ToolOutput::Result reaches wire verbatim (unit+integration) | `cargo test --features full --test tool_output_passthrough --test double_wrap_tripwire --test tool_with_result --test task_augmented_result --test tool_output_result_http` | 9+14+8+11+1 = 43 passed, 0 failed (was 37) | ✓ PASS |
| `double_wrap_tripwire` release-mode never panics | `cargo test --release --features full --test double_wrap_tripwire` | 15 passed, 0 failed | ✓ PASS |
| `wait_for_task` surfaces InputRequired instead of hanging | part of `task_augmented_result` live suite | `wait_for_task_surfaces_input_required_instead_of_hanging` ... ok | ✓ PASS |
| `wait_for_task` timeout not overshot by large server interval | part of `task_augmented_result` live suite | `wait_for_task_timeout_is_not_overshot_by_large_interval` ... ok | ✓ PASS |
| No regression to Phase 101/102 task lifecycle | `cargo test --features full --test tool_as_task_lifecycle --test tool_as_task_lifecycle_http` | 7 + 2 = 9 passed | ✓ PASS |
| Runnable BEFORE/AFTER example | `cargo run --example s47_task_augmented_result --features full` | exit 0, correct BEFORE/AFTER output | ✓ PASS |
| wasm32 lib compiles | `cargo check --target wasm32-unknown-unknown --lib` | success (pre-existing unrelated warnings only) | ✓ PASS |
| `cargo fmt --all -- --check` | — | clean | ✓ PASS |
| `make quality-gate` (Success Criterion 5, explicit) | `make quality-gate` | Full live run: fmt clean, clippy "No lint issues", build OK, 1107+373 tests pass, audit "No vulnerabilities found", zero debt comments, zero production unwraps, ALWAYS validated, purity-check passed. Ends "ALL TOYOTA WAY QUALITY CHECKS PASSED". | ✓ PASS (was FAIL) |
| `make doc-check` (Plan 05's own verification bullet) | `make doc-check` | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features ...` exits 0, "✓ Zero rustdoc warnings" | ✓ PASS (was FAIL) |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| TOUT-01 | 104-01, 104-02, 104-04 | Typed explicit `CallToolResult` return through normal dispatch, sniffing rejected | ✓ SATISFIED | Core mechanism verified; `make quality-gate`/`make doc-check` (touching these files) now both green. |
| TOUT-02 | 104-03 | WARN + debug-fail tripwire on suspicious text-wrap | ✓ SATISFIED | `double_wrap_tripwire`/`looks_like_call_tool_result` verified; WR-02 false-positive precision issue fixed and regression-tested. |
| TOUT-03 | 104-01 | Typed `related_task()` client accessor | ✓ SATISFIED | `related_task()` correct and tested; companion `wait_for_task` CR-01 permanent-hang defect fixed and regression-tested. |
| TOUT-04 | 104-05 | Migration guide + wire-compat proof | ✓ SATISFIED | Guide content complete and accurate (WR-03 correction applied); the plan's own closure gate (`make doc-check` green) now passes. |

No orphaned requirements: ROADMAP.md Phase 104 declares exactly TOUT-01..04, and all four are claimed by at least one of the five plans' `requirements:` frontmatter. (Note: `.planning/REQUIREMENTS.md` in this repo currently tracks only the v2.3/v2.4 Workbook milestone and contains no TOUT-* rows — this is a pre-existing, unrelated repo-tracking gap not introduced by or in scope for Phase 104; requirement traceability for this phase is authoritatively sourced from ROADMAP.md Phase 104's own `Requirements:` line, per the initial verification's approach.)

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| ~~src/server/task_dispatch.rs~~ | ~~211, 256~~ | ~~`clippy::too_long_first_doc_paragraph`~~ | RESOLVED | Fixed by commit f5011384; `make lint` clean. |
| ~~src/types/tools.rs~~ | ~~610, 613, 641~~ | ~~Unresolved intra-doc links / redundant link targets~~ | RESOLVED | Fixed by commit f5011384; `make doc-check` clean. |
| ~~src/server/typed_tool.rs, src/server/mod.rs~~ | ~~796,802 / 254,2834,2837,2843,2844,4017~~ | ~~Redundant explicit rustdoc link targets~~ | RESOLVED | Fixed by commit f5011384; `make doc-check` clean. |
| ~~src/client/mod.rs~~ | ~~670-704~~ | ~~`wait_for_task` unbounded loop on `InputRequired`~~ | RESOLVED | Fixed by commit 688de942; live regression test passes. |
| ~~src/client/mod.rs~~ | ~~691-703~~ | ~~Sleep not clamped to remaining timeout budget~~ | RESOLVED | Fixed by commit 8983f648; live regression test passes. |
| ~~src/server/task_dispatch.rs~~ | ~~234-252~~ | ~~`ContentArray` marker false-positives on chat-message payloads~~ | RESOLVED | Fixed by commit ee2122b4; live regression tests pass (fire + no-fire sides). |
| ~~docs/design/sep-1686-task-augmented-results.md~~ | ~~203-206~~ | ~~Design doc claims gate "naturally passes" when actually bypassed~~ | RESOLVED | Fixed by commit 137995ce; regression test added and passing. |
| ~~src/server/mod.rs~~ | ~~2881~~ | ~~No `tool_with_result_and_description` overload~~ | RESOLVED | Fixed by commit c73d6ee4; new overload added, tested. |
| docs/design/sep-1686-task-augmented-results.md | ~222 (original) | Stray unmatched closing code fence | ℹ️ Info | Cosmetic; out of `fix_scope: critical_warning`, not independently re-checked at exact line — non-blocking, unchanged classification from initial verification. |
| src/server/mod.rs, src/server/core.rs | early-return arms | `result_meta_handle` silently dropped (undrained) on verbatim/create-path/error arms, no debug signal | ℹ️ Info | Unchanged from initial verification; explicitly out of `fix_scope: critical_warning`; not independently blocking. |

No `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER` markers found in any file this phase touched (re-checked live across all 9 modified source files in this session: src/server/task_dispatch.rs, src/server/mod.rs, src/server/core.rs, src/server/typed_tool.rs, src/server/cancellation.rs, src/types/tools.rs, src/types/tasks.rs, src/client/mod.rs, docs/design/sep-1686-task-augmented-results.md).

### Human Verification Required

None. All must-haves for this phase are mechanically verifiable (Rust type/dispatch plumbing with unit/integration/property/live-HTTP tests and `make`-target build gates); no UI, visual, or subjective-UX surface exists in this phase's scope.

### Gaps Summary

All 3 gaps from the prior verification (`104-VERIFICATION.md` initial run, status `gaps_found`, score 4/7) are closed and independently re-confirmed in this session, not accepted on the SUMMARY's/REVIEW-FIX's word alone:

1. **`make quality-gate` is now green.** Re-ran the full command live (not a narrower `cargo clippy --lib` substitute) and captured the complete ~5360-line log: formatting clean, clippy zero issues under the CI-matching pedantic/nursery superset, build succeeds, 1107 lib + 373 integration tests pass, audit clean (with a well-documented, correctly-scoped new ignore for an unrelated dependency), zero debt markers, zero production unwraps, ALWAYS requirements validated (all 44 examples build, including s47), purity-check passes. The two `clippy::too_long_first_doc_paragraph` errors are gone — confirmed both by the live clean run and by inspecting the diff that reflowed the two flagged doc comments.

2. **`Client::wait_for_task`'s Critical hang defect (CR-01) is fixed and regression-tested.** The poller now returns `Error::Validation` immediately when a task enters `InputRequired` instead of spinning forever under `WaitForTaskOptions::default()` — the exact usage pattern the method's own rustdoc and the pmcp-book chapter recommend. A live duplex test that drives a real store task `Working -> InputRequired` server-side and asserts a prompt error (not a hang) passes. The companion WR-01 timeout-overshoot issue is also fixed (interval now clamped to the remaining budget) and regression-tested.

3. **`make doc-check` is now green.** Re-ran the exact command live: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features ...` exits 0 with "Zero rustdoc warnings" — the 12 prior errors (unresolved intra-doc links + redundant explicit link targets) are gone, confirmed both by the live clean run and by inspecting the fix diff.

Additionally, all 4 code-review Warnings (WR-01 through WR-04) that were tracked (not independently blocking) in the initial verification have also been fixed in the same remediation pass, each with a live-passing regression test: WR-01 (timeout clamp), WR-02 (tripwire false-positive precision on chat-message payloads), WR-03 (design-doc correction + gate-precedence regression test), WR-04 (`tool_with_result_and_description` builder overload). No regressions were found in any previously-passing behavior: the verbatim HTTP wire test, the double-wrap tripwire (both debug and release modes), the `related_task()` round-trip, the Phase 101/102 task-lifecycle regression suite, the wasm32 library compile, and the runnable BEFORE/AFTER example all re-ran clean in this session.

The phase goal is achieved: a `ToolHandler` can return a full, task-augmented `CallToolResult` through normal `Server`/`ServerCore` dispatch, un-re-wrapped, with a high-precision double-wrap tripwire, a typed client accessor, and a complete, gate-passing migration guide — with zero known regressions and all explicit quality gates (`make quality-gate`, `make doc-check`) passing live.

---

_Verified: 2026-07-05_
_Verifier: Claude (gsd-verifier)_
