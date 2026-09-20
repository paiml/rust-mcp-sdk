---
phase: 121-local-round-trip-e2e
plan: 03
subsystem: testing
tags: [pkg-04, roundtrip, negative-tests, structural-guard, raii, rustdoc, oci, slots]

# Dependency graph
requires:
  - phase: 121-local-round-trip-e2e (plan 01)
    provides: "`tests/common/` shared module — `EnvVarGuard` (panic-safe RAII env restore), `tfl_env_lock`, `fixtures_dir`, `mount_london_tube`, `DUMMY_APP_KEY`; the `test-openapi-server` Makefile target chained into `test-all`"
  - phase: 121-local-round-trip-e2e (plan 02)
    provides: "`compare_tool_surfaces` + `SurfaceMismatch`, `capture_tool_surface`, `pack_a_and_move_to_b`, `serve_environment`, `EXPECTED_TOOL_NAMES`, and the four positive round-trip tests"
  - phase: 120-config-server-packaging
    provides: "`Endpoint`/`AuthMode` made behaviour-relevant in `classify` — the evidence for Task 3's rustdoc correction"
provides:
  - "SC4-red: two negative tests proving the round trip goes RED on a real regression, each asserting the SPECIFIC failure rather than merely that something failed"
  - "A panic-safe RAII proof (`env_var_guard_restores_prior_state_including_on_panic`) — the first thing in this crate to actually EXECUTE plan 121-01's `EnvVarGuard`"
  - "SC4-green: a machine-checked structural guard that scans delimiter-balanced assertion SPANS, so the rustfmt-normal multiline shape is covered"
  - "`detect_deviation`'s rustdoc now matches its behaviour (CONTEXT roadmap correction #2)"
affects: [phase-122, phase-124, manifest-shape-refactors, pmcp-package-slot-docs]

actuals:
  tokens: 21652
  tasks: 3
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Assertion-SPAN scanning: a delimiter-depth accumulator carried ACROSS lines, so a lint over source text sees the shape rustfmt actually produces"
    - "Guard-of-the-guard self-checks: a measured-and-derived floor plus a multiline-coverage inequality that fails if the scanner degenerates"
    - "Two-part negative assertions (specific variant via `matches!`/`match` PLUS the `Display` message naming the degraded identifier), lifted from `pmcp-package/tests/negative.rs`"
    - "Full nested error-path matching across three `#[non_exhaustive]` enums with a bound inner field"

key-files:
  created: []
  modified:
    - crates/pmcp-openapi-server/tests/roundtrip_e2e.rs
    - crates/pmcp-package/src/slot/deviation.rs
    - .planning/phases/121-local-round-trip-e2e/deferred-items.md

key-decisions:
  - "Both arguments to the missing-tool negative come from ONE real capture, making the degradation the SOLE variable by construction; capturing twice would admit a second source of difference into a test whose purpose is attribution"
  - "The missing-tool negative's non-vacuity is asserted as a PRECONDITION over the full surface, not as arithmetic over the removal — a length check would have fired first and masked the `expect_err` the inversion proof has to land on"
  - "`TFL_APP_KEY` stays FILLED in the unfilled-slot negative: `dispatch` builds the auth provider BEFORE resolving the endpoint, so unsetting both risks failing at `DispatchError::Auth` and never reaching the nested pattern"
  - "The span scanner sanitizes CHAR literals as well as string literals and comments — this file writes `'('` and `'\"'` in the scanner itself, and either would corrupt the scan of the file the guard reads"
  - "The sanitizer carries string state ACROSS lines: this repo's assertion messages are routinely `\\`-continued, and a per-line sanitizer would treat a continuation line as code"
  - "D-09 floor DERIVED from a measurement (42 spans) less a stated 23.8% margin = 32, with both numbers written into the comment"
  - "`SlotClass`'s identically-stale variant doc was NOT fixed — outside this plan's `files_modified`; logged as deferred item D3"

patterns-established:
  - "Span-based lexical lints over own source via `include_str!` of the file's own name"
  - "Every machine-checked guard ships with a proof that it FAILS when its subject is degraded AND a proof that it fails rather than passes when it scans nothing"

requirements-completed: [PKG-04]

coverage:
  - id: D1
    description: "Dropping one named tool from environment B's served surface turns the round trip RED with an error naming that specific tool"
    requirement: PKG-04
    verification:
      - kind: integration
        ref: "crates/pmcp-openapi-server/tests/roundtrip_e2e.rs#degraded_env_b_missing_tool_is_reported"
        status: pass
      - kind: integration
        ref: "inversion proof: degradation made a no-op -> test FAILS at expect_err with Ok(())"
        status: pass
    human_judgment: false
  - id: D2
    description: "Leaving environment B's endpoint slot unfilled fails assembly with the FULL nested RunError::Dispatch(DispatchError::UnresolvedBaseUrl(ToolkitError::UnresolvedBaseUrlRef { var })) shape, var == TFL_BASE_URL"
    requirement: PKG-04
    verification:
      - kind: integration
        ref: "crates/pmcp-openapi-server/tests/roundtrip_e2e.rs#degraded_env_b_unfilled_slot_is_reported"
        status: pass
      - kind: integration
        ref: "specificity mutation: Dispatch(_) PASSES on Dispatch(MissingBackend) while the nested pattern FAILS on it"
        status: pass
      - kind: integration
        ref: "inversion proof: variable left SET -> assembly succeeds -> test FAILS at expect_err"
        status: pass
    human_judgment: false
  - id: D3
    description: "The RAII env guard the unfilled-slot negative's isolation rests on restores prior state in BOTH directions after a panicking body"
    requirement: PKG-04
    verification:
      - kind: unit
        ref: "crates/pmcp-openapi-server/tests/roundtrip_e2e.rs#env_var_guard_restores_prior_state_including_on_panic"
        status: pass
      - kind: unit
        ref: "inversion proof: trailing-restore shape substituted -> test FAILS, variable not restored"
        status: pass
      - kind: integration
        ref: "cross-check: cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1 still reports exactly 3 passed / 1 ignored"
        status: pass
    human_judgment: false
  - id: D4
    description: "roundtrip_e2e.rs is machine-checked to contain no assertion on manifest field names, layer ordering or digest values, across whole assertion SPANS"
    requirement: PKG-04
    verification:
      - kind: unit
        ref: "crates/pmcp-openapi-server/tests/roundtrip_e2e.rs#roundtrip_e2e_asserts_nothing_about_manifest_shape"
        status: pass
      - kind: unit
        ref: "teeth proof MULTILINE: token planted on a continuation line -> guard FAILS naming token + start line"
        status: pass
      - kind: unit
        ref: "teeth proof single-line, string-literal sanitizing, floor (0 spans), and collapse self-check — all four recorded"
        status: pass
    human_judgment: false
  - id: D5
    description: "detect_deviation's rustdoc describes what the function does; make pmcp-package-gate green; no non-comment line changed"
    verification:
      - kind: other
        ref: "make pmcp-package-gate (fmt + clippy -D warnings + 246 tests incl. 8 doctests) exits 0"
        status: pass
      - kind: other
        ref: "git diff crates/pmcp-package/src/slot/deviation.rs — all 34 changed lines start with ///"
        status: pass
    human_judgment: true
    rationale: "Prose accuracy against `classify`'s real behaviour is a reading judgment. The gate proves the crate still compiles, lints and tests clean, and the diff proves no code changed — but that the corrected text is TRUE is something a human should confirm against classification.rs:25-29."

# Metrics
duration: 29 min
completed: 2026-08-24
status: complete
---

# Phase 121 Plan 03: SC4 Both Directions Summary

**Two negative tests that turn the round trip red on a real regression (a dropped tool; an unfilled endpoint slot matched three enums deep to the `TFL_BASE_URL` variable name), a RAII proof the env-guard actually restores after a panic, and a span-scanning structural guard that machine-checks the file asserts nothing about manifest shape — every one demonstrated to fail when its subject is degraded.**

## Performance

- **Duration:** 29 min
- **Started:** 2026-08-24T00:43:24Z
- **Completed:** 2026-08-24T01:11:54Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- **SC4-red is now real.** `roundtrip_e2e` went from 4 tests nobody had ever seen fail to 8, three of which exist to prove the other five bite. Both negatives assert the SPECIFIC failure — variant plus `Display` message — and both were run with their degradation removed to confirm they go green only when there is genuinely nothing wrong.
- **SC4-green is machine-checked across assertion SPANS, not lines.** The guard was demonstrated to catch a deny-listed token planted on a rustfmt-produced CONTINUATION line — the exact bypass a per-line scan cannot see, and the proof the earlier plan draft could not make.
- **The unfilled-slot negative is matched three enums deep.** A specificity mutation showed `RunError::Dispatch(_)` passing on an unrelated `MissingBackend` failure while the full nested pattern rejects it — so the loose form is provably not good enough.
- **`EnvVarGuard` is now executed, not just shipped.** Plan 121-01 created it but deliberately left it untested; this plan is the first thing in the crate to run it, in both restore directions, after a panic.
- **`detect_deviation`'s rustdoc matches its behaviour**, with `make pmcp-package-gate` green and no non-comment line touched.

## Task Commits

1. **Task 1: negative tests over a degraded environment B (D-08, SC4-red)** — `0dc1e25b` (test)
2. **Task 2: the D-09 structural guard over assertion SPANS (SC4-green)** — `233250c7` (test)
3. **Task 3: correct `detect_deviation`'s stale rustdoc** — `cb9a2f1d` (docs)

## Files Created/Modified

- `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` — +655 lines: four new test functions, the deny-list constant, the `include_str!` self-source const, the span scanner (line sanitizer + cross-line delimiter accumulator + span cap), and the floor/coverage self-checks.
- `crates/pmcp-package/src/slot/deviation.rs` — rustdoc only (34 changed lines, every one `///`).
- `.planning/phases/121-local-round-trip-e2e/deferred-items.md` — new deferred item D3.

## Proof Record

Every mutation, inversion and teeth proof required by the plan, with the verbatim message of the assertion that fired.

### The D-09 measurement

- **Measured assertion-SPAN count: 42** (read by temporarily raising the floor to 99999 and letting the floor assertion report the real number).
- **Derived floor: 32** — 42 less a margin of 10, i.e. **23.8% below**, inside the "no more than a quarter" bound and above the absolute minimum of 10. Both numbers are written into the constant's doc comment so the next person can re-derive rather than nudge.

### Teeth proof — deny-list, MULTILINE (the proof the earlier draft could not make)

Planted an executable three-line `assert_eq!` with `planted_shape::sha256::value()` on a **continuation line carrying no assertion keyword**. Guard **FAILED**:

> `this file asserts on the package's ON-DISK REPRESENTATION: the forbidden token 'sha256:' appears in the assertion span starting at line 1684. Every assertion here must be on SERVED BEHAVIOUR — the round trip has to survive an arbitrary number of manifest-shape refactors (PKG-04 / SC4-green, D-09). span:`
> ```
>     assert_eq!(
>         7,
>         planted_shape::sha256::value(),
>
>     );
> ```

The message names both the offending token and the span's starting line, and the printed span shows the string-literal argument already sanitized to blank. Plant removed → guard **PASSES**.

### Teeth proof — deny-list, single line

Token on the macro's own line (`assert_eq!(planted_shape::sha256::value(), 7);`). Guard **FAILED**, same message shape, `span: assert_eq!(planted_shape::sha256::value(), 7);`. The easy case is still caught. Plant removed → **PASSES**.

### Teeth proof — string-literal sanitizing

Planted an assertion whose failure MESSAGE contains `sha256:deadbeef`, an unbalanced `(`, and the word `digest`, followed by a second assertion. Both required outcomes held:

- The guard did **NOT** trip on the tokens inside the string — full suite **9 passed, exit 0**.
- The span **closed correctly**: the span count read **44**, i.e. 42 + 2. Both planted assertions were counted as SEPARATE spans, so the unbalanced paren inside the string did not swallow the following assertion (a runaway would have yielded 43 or fewer).

### Teeth proof — floor

Span-start detection temporarily changed to require a substring that never occurs. Guard **FAILED** rather than passing on an empty scan:

> `the manifest-shape guard IS NOT REACHING THE FILE: it completed only 0 assertion spans, below the floor of 32. Something upstream of the deny-list check is broken — a renamed file, an over-tight span-start filter, or an include_str! pointing somewhere unexpected — and the guard would otherwise pass having examined nothing.`

Reverted → **PASSES**.

### Teeth proof — collapse self-check

Accumulator disabled so every span ends on its start line. The floor still passed (42 ≥ 32) and the failure landed exactly on the coverage inequality, as designed:

> `the span scanner COLLAPSED TO SINGLE LINES (42 covered lines across 42 spans): every span ended on its own start line, which means the cross-line accumulator stopped working and THE MULTILINE BYPASS IS BACK — a forbidden token on a rustfmt-produced continuation line would no longer be examined.`

Reverted → **PASSES**.

### Specificity mutation — `Dispatch(_)` vs the full nested pattern

Assembly was made to fail a DIFFERENT way that still lands in `Dispatch`: the `[backend]` section was stripped from a COPY of the config in a `TempDir`, yielding `DispatchError::MissingBackend`.

- **Loosened pattern `RunError::Dispatch(_)`: test PASSED** (`test result: ok. 1 passed`) — on a failure that has nothing to do with an unfilled slot.
- **Full nested pattern: test FAILED** on the same unrelated failure:
  > `expected RunError::Dispatch(DispatchError::UnresolvedBaseUrl(ToolkitError::UnresolvedBaseUrlRef { .. })), got Dispatch(MissingBackend)`

This is the proof that the loose form is not good enough. Full pattern restored.

### Inversion — missing-tool negative is not vacuous

Degradation made a no-op (filter removes nothing). Test **FAILED** because the comparison returned `Ok`:

> `dropping a tool from environment B's served surface must be REPORTED — a comparison that tolerates it is a round trip nobody has ever seen fail (PKG-04 / SC4-red): ()`

The trailing `: ()` is the `Ok` payload, confirming the failure is "comparison succeeded" and not something else. Degradation restored.

### Inversion — unfilled-slot negative is not vacuous

`TFL_BASE_URL` left SET. Test **FAILED** because assembly succeeded:

> `environment B must REFUSE to assemble while its endpoint slot is unfilled — assembling anyway would serve a nonsense URL (PKG-04 / SC4-red): (127.0.0.1:54145, JoinHandle { id: Id(2) })`

The payload is a real bound address and serving handle. Restored.

### Inversion — guard restoration

Inner `EnvVarGuard::set` replaced with a bare `std::env::set_var` plus a trailing restore line inside the panicking closure. Test **FAILED** because the variable was NOT restored:

> `a previously-UNSET variable must be restored to unset even after a panic — a trailing restore line would have been skipped here, and the variable would leak into every later test in this binary`

Guard restored. This is the direct, executed evidence for the plan's prohibition on trailing restore lines.

### Self-satisfaction proof

Both required cases hold while the guard is **green**:

- A whole comment line containing a deny-list token: `roundtrip_e2e.rs:31` — `//! a layer's position or count, not a digest value.`
- A `//` comment containing a deny-list token on a line **INSIDE an assertion span**: `roundtrip_e2e.rs:1225` — inside the `assert!(` span that starts at line 1223. This proves the sanitizer strips comments WITHIN spans, not merely whole comment lines.

## Verification

| # | Check | Result |
|---|-------|--------|
| 1 | `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` | exit 0, **8 passed** (4 positives + 2 negatives + RAII proof + structural guard) |
| 2 | `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` | exit 0, **exactly 3 passed, 1 ignored** — no env leakage across the binary |
| 3 | `cargo test -p pmcp-openapi-server --test pmcp_package_pin -- --test-threads=1` | exit 0, 1 passed |
| 4 | `make test-openapi-server` | exit 0, **41 tests** (planner measured 32 before this phase → +9) |
| 5 | `make pmcp-package-gate` | exit 0 — fmt + `clippy -D warnings` + 246 tests incl. 8 doctests |
| 6 | `make quality-gate` | **exit 0** — and its log confirms both `✓ pmcp-openapi-server tests passed (41 tests)` and `✓ pmcp-package fmt/clippy/test OK`, so this phase's deliverable really is executed by the gate |
| 7 | `git status --porcelain` | clean of proof artifacts (see below) |
| 8 | Repeated-run leak check | `roundtrip_e2e` passed twice in a row in the same tree, then `make test-openapi-server` passed immediately afterwards |

**`make test-openapi-server` REQUIRED_TEST_BINARIES:** `parity_replay pmcp_package_pin roundtrip_e2e` — all three confirmed present in the run output (`tests/parity_replay.rs`, `tests/pmcp_package_pin.rs`, `tests/roundtrip_e2e.rs`). The binary list was re-checked after an initial grep of mine used `[a-z_]*`, which silently excluded `roundtrip_e2e` because of the digit in `e2e` — a false negative in my own verification command, not in the gate.

**Proof-artifact cleanliness:** `grep -c 'planted\|TEMPORARY MUTATION\|TEETH PROOF'` over `roundtrip_e2e.rs` is **0**; `git status --porcelain crates/pmcp-openapi-server/tests/fixtures/` is **empty**; the floor constant reads its real derived value (`= 32`, not the 99999 measurement probe). Every mutation was reverted against a SHA-256-verified pristine copy.

## Decisions Made

- **Both sides of the missing-tool comparison come from one real capture.** Environment B is served for real (restored config, own backend, own credential) and its surface captured; the full capture is A's side and the same capture minus `get-tube-status` is B's. This makes the degradation the sole variable by construction. That A's and B's independently-captured surfaces are set-equal is a different claim, proven separately by `roundtrip_tool_surface_parity`.
- **Non-vacuity asserted as a precondition, not as arithmetic.** An `assert_eq!(degraded.len() + 1, full.len())` would have been the obvious guard, but it fires FIRST when the degradation is made a no-op — masking the `expect_err` that the plan's inversion proof must land on. Asserting instead that the full surface CONTAINS the tool is unaffected by the filter, so the inversion lands exactly where it should.
- **`TFL_APP_KEY` stays filled in the unfilled-slot negative.** `dispatch` constructs the outgoing auth provider (`src/dispatch.rs:129`) before resolving the endpoint (`:142`), so unsetting both risks `DispatchError::Auth` and the nested pattern would never be reached. Exactly one slot is degraded.
- **The span sanitizer also strips CHAR literals and carries string state across lines.** Neither is optional here: the scanner's own source writes `'('` and `'"'` as char literals (which would be miscounted as delimiters, or misread as a string opener), and this repo's assertion messages are routinely `\`-continued across lines, so a per-line sanitizer would treat continuation text as code and count its parentheses.
- **`SlotClass`'s doc was left alone.** It carries the identical staleness one level up, but `classification.rs` is outside this plan's `files_modified`; logged as deferred item D3.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The new tester was not initialized before `list_tools`**
- **Found during:** Task 1 (`degraded_env_b_missing_tool_is_reported`)
- **Issue:** `serve_environment`'s readiness loop initializes its own throwaway `ServerTester`; each tester carries its own session state, so the test's tester was uninitialized and `capture_tool_surface`'s first guard fired with `error=Some("Client not initialized - please run initialize test first")`.
- **Fix:** Added the `test_initialize()` assertion before capture, exactly as `roundtrip_tool_surface_parity` does, with a comment explaining why the probe's initialization does not carry over.
- **Files modified:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs`
- **Verification:** Suite went 6 passed/1 failed → 7 passed.
- **Committed in:** `0dc1e25b` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking).
**Impact on plan:** None on scope. The fix is two lines of setup the plan's own reference test already performs; it was caught by a wave-2 guard doing exactly its job.

## Issues Encountered

- **`rtk` output corruption, as the prior-wave handoff warned.** `git status --porcelain` returned the literal string `ok` and `git commit` returned `ok worktre`. Every measurement in this summary was therefore taken with absolute binary paths (`/usr/bin/git`, `/Users/guy/.cargo/bin/cargo`, `/usr/bin/make`, `/usr/bin/grep`) or by redirecting to a file and reading it back.
- **`cargo clippy -p pmcp-openapi-server --all-targets -- -D warnings` still exits 101** on the pre-existing `mcp-tester` `manual_filter` lints — unchanged from waves 1 and 2, already recorded as deferred item D1. Verified that the intent holds: `cargo clippy -p pmcp-openapi-server --all-targets` (letting the dependency compile) exits **0** with **zero** warnings mentioning `roundtrip_e2e.rs`. Not fixed — outside this plan's `files_modified`, and `make quality-gate` (the real gate) is green.
- **`.pmat/` cache files show as modified** after the gate run (`context.db`, `context.idx/manifest.json`, plus the three already dirty at session start). These are pmat's analysis caches regenerated by running `make quality-gate`, not proof artifacts and not part of this plan — deliberately left uncommitted.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- **PKG-04's regression net is complete and gated.** SC1–SC3 (waves 1–2) plus SC4 in both directions (this wave) all run inside `make quality-gate`, which is what CI's `quality-gate` job executes and what `gate.needs` makes merge-blocking.
- **The durability claim is now enforced rather than promised.** A future manifest-shape refactor that tries to couple this file to on-disk representation fails at `roundtrip_e2e_asserts_nothing_about_manifest_shape` with a message naming the token and the line — but note the guard's own header: it is a lexical regression lint, so reviewer attention remains part of SC4-green.
- **Two carry-forwards for a later pass, neither blocking:** deferred item D1 (pre-existing dependency clippy lints) and new deferred item D3 (`SlotClass`'s variant doc is stale in the same way `detect_deviation`'s was).
- **If the floor ever goes red after a legitimate assertion refactor,** re-measure by temporarily raising `MANIFEST_SHAPE_GUARD_SPAN_FLOOR` and re-apply the same ≤25% margin. Do not nudge the number to make a run go green — that is the failure mode the written-down derivation exists to prevent.

## Self-Check: PASSED

**Files claimed, verified present on disk:**
- `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` (76863 bytes) — FOUND
- `crates/pmcp-package/src/slot/deviation.rs` (5725 bytes) — FOUND
- `.planning/phases/121-local-round-trip-e2e/121-03-SUMMARY.md` — FOUND
- `.planning/phases/121-local-round-trip-e2e/deferred-items.md` — FOUND

**Commits claimed, verified in `git log --oneline --all`:**
- `0dc1e25b` test(121-03): negative tests for a degraded environment B — FOUND
- `233250c7` test(121-03): D-09 structural guard over assertion SPANS — FOUND
- `cb9a2f1d` docs(121-03): correct detect_deviation's stale rustdoc — FOUND

The fourth commit — `docs(121-03): complete SC4 both-directions plan`, carrying this
file — is deliberately identified by its message rather than by a hash. This section was
folded into that commit by `--amend`, which rewrites the hash it would have quoted, so any
hash written here would name a commit that no longer exists. The three task commits above
are stable and were verified by hash.

**Acceptance criteria re-run at close-out:** all eight `<verification>` items in the
table above were executed, not inferred. `make quality-gate` — the phase gate — exits 0
and its log names both `✓ pmcp-openapi-server tests passed (41 tests)` and
`✓ pmcp-package fmt/clippy/test OK`.

**Ledger:** no stubs, no skipped tests, and no unrun `<verify>` blocks were produced by
this plan, so there is nothing to append to `.planning/WINDOWS.md`. The one deviation was
fixed and verified within its own task commit rather than deferred.

---
*Phase: 121-local-round-trip-e2e*
*Completed: 2026-08-24*
