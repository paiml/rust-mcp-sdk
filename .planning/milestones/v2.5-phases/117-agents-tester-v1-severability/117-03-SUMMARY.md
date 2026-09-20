---
phase: 117-agents-tester-v1-severability
plan: 03
subsystem: testing
tags: [mcp-tester, golden-fixtures, additivity, report-format, byte-identity, dual-run, hashmap-nondeterminism]

# Dependency graph
requires:
  - phase: 78-mcp-apps-guide-anchors
    provides: "`TestReport::print_to_writer` (`report.rs:244-255`) — the writer seam added by Plan 78-04 so tests can assert PRINTED bytes rather than only parsed structures"
  - phase: 115-json-schema-2020-12-structured-output-caching-hints
    provides: "`tests/v1_lists_golden.rs:97-186` — the width-preserving normalizer (`DynamicField` / `substitute` / `key_occurrences`) and the four-step `assert_v1_bytes` shape, restated here"
provides:
  - "`crates/mcp-tester/tests/report_compat.rs` — 7 tests pinning single-run mcp-tester 0.7.0 report output BEFORE any `--dual-run` code exists"
  - "A byte-for-byte `--format json` golden with proven width-preserving normalization (`timestamp` is the only dynamic)"
  - "A byte-for-byte `--format pretty` golden on a deliberately-pinned single-category / zero-duration / ANSI-off fixture, plus an order-insensitive MULTISET assertion on a multi-category fixture"
  - "The recorded A2 re-measurement (three greps, run verbatim) that decides what plan 117-11 is allowed to add"
  - "The recorded `cargo build -p cargo-pmcp` pre-change baseline that 117-11 must still satisfy"
  - "A SECOND `TestResult` struct literal (`cargo-pmcp/src/commands/test/check.rs:522`) that A-D11 did not name, and the observation that `cargo build` cannot see it"
affects: [117-11, 117-12, 117-13, 117-14, mcp-tester-dual-run, cargo-pmcp]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Capture-then-splice golden authoring: goldens are machine-transcribed from a temporary dump test, never hand-counted, so column padding cannot silently drift"
    - "Achievable-criterion pinning: eliminate the non-determinism you can (ANSI, duration column, single-entry HashMap) and assert structurally ONLY over what remains"
    - "Multiset-not-set comparison for order-insensitive output, with the set-would-pass claim proven by an executed control rather than asserted"
    - "Cite-both-ranges convention: where a plan's line citation has drifted from HEAD, record the plan's range AND the measured range instead of silently correcting"

key-files:
  created:
    - crates/mcp-tester/tests/report_compat.rs
  modified: []

key-decisions:
  - "Captured against the UNMODIFIED 0.7.0 crate: `git diff --stat crates/mcp-tester/src/ crates/mcp-tester/Cargo.toml` is empty, so the goldens describe the shipped binary and not a co-evolved one"
  - "`duration` gets NO dynamic — serde renders `std::time::Duration` as a `{secs, nanos}` struct, fully deterministic at `Duration::from_secs(0)`; proven by an executed test, not asserted in prose"
  - "The fixture INTERLEAVES categories, so the pretty printer's regrouping is a real transformation and the intra-block ordering assertion is not a tautology over one-element blocks"
  - "The single-category pretty fixture deliberately contains a FAILURE, so the golden also pins the `RECOMMENDATIONS` block and `Overall Status: FAILED` rather than only the happy path"
  - "ANSI is pinned off with an explicit `colored::control::set_override(false)` in every test rather than inherited from tty detection, which differs between a terminal, a CI runner and a captured `Vec<u8>`"
  - "The negative control duplicates a RENDERED line, not a fixture entry: duplicating a fixture entry also moves the summary counters, so a set comparison would catch it too and the control would not isolate multiplicity"
  - "`cargo check -p cargo-pmcp --tests` recorded ALONGSIDE `cargo build -p cargo-pmcp`, because the newly-found second `TestResult` literal lives in a `#[cfg(test)]` module that `cargo build` does not compile"

patterns-established:
  - "Additivity proof by pre-change capture: the baseline is committed before the feature branch exists, so 'unchanged' is a diff and not a memory"
  - "Failure messages name the REMEDY (move it behind `--dual-run`, or add a new top-level struct like `post_deploy_report`) and explicitly forbid re-recording the golden"

requirements-completed: [CLNT-04]

# Metrics
duration: 48min
completed: 2026-08-07
---

# Phase 117 Plan 03: mcp-tester Single-Run Report Compat Summary

**The single-run report output of mcp-tester 0.7.0 is now pinned by 7 executable tests — a byte golden for `--format json`, a byte golden for a deliberately-pinned single-category `--format pretty` fixture, and an order-insensitive multiset for the multi-category case — so plan 117-11's `--dual-run` additivity claim is a diff rather than a promise.**

## Performance

- **Duration:** ~48 min
- **Tasks:** 2 of 2
- **Files created:** 1 (`crates/mcp-tester/tests/report_compat.rs`, 771 lines)
- **Files modified:** 0
- **`mcp-tester` source touched:** none

## Accomplishments

### Task 1 — `--format json` pinned byte-for-byte (commit `39bda1d2`)

`print_json` (`report.rs:460-463`) emits `serde_json::to_string_pretty(&TestReport)` verbatim, so the binary's json contract IS the serde shape of `TestReport`. The golden pins those bytes for a 6-test, 3-category, 4-status, deliberately interleaved fixture.

The normalizer (`DynamicField` / `width_preserving` / `substitute` / `key_occurrences`) is **restated** from `tests/v1_lists_golden.rs:97-186`, not imported — a Rust integration test is its own crate. One adaptation was required: the needle is `"key": "` (with the space) because this file normalizes pretty-printed JSON, whereas `v1_lists_golden.rs` normalizes compact wire bytes.

The assertion follows the four-step `assert_v1_bytes` shape: same-width substitution → length equality → per-key occurrence equality → canonical substitution → `assert_eq!`.

### Task 2 — `--format pretty` pinned by an achievable criterion (commit `6d829e2d`)

Two distinct `#[test]` functions, exactly as the plan required:

- `pretty_single_category_output_is_byte_pinned` — a real BYTE golden. All three non-determinism sources are eliminated by construction: one category means a one-entry `HashMap` with exactly one iteration order; all durations are zero so the `>100ms` column never fires; ANSI is pinned off.
- `pretty_multi_category_line_multiset_is_pinned` — a `BTreeMap<String, usize>` occurrence count, plus `pretty_multi_category_blocks_are_internally_ordered` asserting insertion order *within* each block. The only thing left unasserted is the order **of** the category blocks.

Stability was measured, not assumed: **8 consecutive runs, 7 passed / 0 failed every time**, under Rust's per-process randomly-seeded hasher.

## Recorded Measurements

### Did the serde `Duration` need a dynamic? **NO.**

`std::time::Duration` serializes as a serde *struct*, not a string:

```json
"duration": {
  "secs": 0,
  "nanos": 0
}
```

Every fixture pins it to `Duration::from_secs(0)` — the same choice `TestReport::from_error` already makes at `report.rs:197` — so it renders identically on every run. `JSON_DYNAMICS` therefore carries exactly one entry (`timestamp`). This is not a prose claim: `json_duration_is_deterministic_without_a_dynamic` asserts the `{secs, nanos}` shape AND that two captures agree once `timestamp` alone is normalized, so if `Duration`'s serde representation ever changes, the test fails and the dynamic list must be revisited.

### A2 re-measurement — commands RUN, output verbatim

**Command 1**

```
grep -rn 'TestReport *{' --include='*.rs' cargo-pmcp/ crates/ src/ tests/ | grep -v 'crates/mcp-tester/'
```

```
cargo-pmcp/src/loadtest/report.rs:27:pub struct LoadTestReport {
cargo-pmcp/src/loadtest/report.rs:163:impl LoadTestReport {
cargo-pmcp/src/commands/test/apps.rs:390:) -> TestReport {
```

All three are non-literals: two are the declaration and `impl` block of `LoadTestReport`, a *different* type that the pattern matches only as a suffix; the third is a function *return type* (`fn run_source_scan_json(...) -> TestReport {`), not a struct literal.

**Command 2**

```
grep -rn 'TestResult *{' --include='*.rs' cargo-pmcp/ | head
```

```
cargo-pmcp/src/loadtest/report.rs:366:    fn test_result() -> LoadTestResult {
cargo-pmcp/src/loadtest/report.rs:367:        LoadTestResult {
cargo-pmcp/src/loadtest/engine.rs:280:        Ok(LoadTestResult {
cargo-pmcp/src/loadtest/engine.rs:483:        Ok(LoadTestResult {
cargo-pmcp/src/loadtest/engine.rs:494:pub struct LoadTestResult {
cargo-pmcp/src/loadtest/engine.rs:888:        let result = LoadTestResult {
cargo-pmcp/src/loadtest/summary.rs:319:        let result = LoadTestResult {
cargo-pmcp/src/loadtest/summary.rs:342:        let result = LoadTestResult {
cargo-pmcp/src/loadtest/summary.rs:359:        let result = LoadTestResult {
cargo-pmcp/src/loadtest/summary.rs:393:        let result = LoadTestResult {
```

**⚠ The `| head` in this command is MISLEADING and nearly cost the plan its finding.** All ten visible hits are `LoadTestResult` noise; `head` truncates before the real ones. Re-run without it (filtering the unrelated type):

```
grep -rn 'TestResult *{' --include='*.rs' cargo-pmcp/ | grep -v 'LoadTestResult'
```

```
cargo-pmcp/src/commands/test/apps.rs:874:fn make_read_failure_result(uri: &str, reason: &str) -> mcp_tester::TestResult {
cargo-pmcp/src/commands/test/apps.rs:875:    mcp_tester::TestResult {
cargo-pmcp/src/commands/test/check.rs:521:    fn mk_test(name: &str, status: TestStatus, error: Option<String>) -> TestResult {
cargo-pmcp/src/commands/test/check.rs:522:        TestResult {
```

**Command 3**

```
grep -rn 'TestCategory::' --include='*.rs' cargo-pmcp/src/commands/test/conformance.rs
```

```
cargo-pmcp/src/commands/test/conformance.rs:138:        ("Core", TestCategory::Core),
cargo-pmcp/src/commands/test/conformance.rs:139:        ("Transport", TestCategory::Transport),
cargo-pmcp/src/commands/test/conformance.rs:140:        ("Tools", TestCategory::Tools),
cargo-pmcp/src/commands/test/conformance.rs:141:        ("Resources", TestCategory::Resources),
cargo-pmcp/src/commands/test/conformance.rs:142:        ("Prompts", TestCategory::Prompts),
cargo-pmcp/src/commands/test/conformance.rs:143:        ("Tasks", TestCategory::Tasks),
cargo-pmcp/src/commands/test/conformance.rs:278:        TestCategory::Core => "core",
cargo-pmcp/src/commands/test/conformance.rs:279:        TestCategory::Transport => "transport",
cargo-pmcp/src/commands/test/conformance.rs:280:        TestCategory::Tools => "tools",
cargo-pmcp/src/commands/test/conformance.rs:281:        TestCategory::Resources => "resources",
cargo-pmcp/src/commands/test/conformance.rs:282:        TestCategory::Prompts => "prompts",
cargo-pmcp/src/commands/test/conformance.rs:283:        TestCategory::Tasks => "tasks",
cargo-pmcp/src/commands/test/conformance.rs:285:        TestCategory::Protocol
cargo-pmcp/src/commands/test/conformance.rs:286:        | TestCategory::Performance
cargo-pmcp/src/commands/test/conformance.rs:287:        | TestCategory::Compatibility
cargo-pmcp/src/commands/test/conformance.rs:288:        | TestCategory::Apps => "core",
```

The `:278-288` match covers all **10** `TestCategory` variants (6 named arms + a 4-variant or-pattern) with **no `_` arm**, so adding a variant is a hard compile break. Confirmed.

### Verdict: **A2 CONFIRMED** — with a material addendum

**A2 CONFIRMED.** No `TestReport` struct literal exists anywhere outside `crates/mcp-tester/`.

**Decision input for plan 117-11:** an optional `#[serde(default, skip_serializing_if = "Option::is_none")]` field on `TestReport` **WOULD** be safe — nothing outside the crate constructs one positionally, so no external code would fail to compile. **117-11 should nevertheless take the stronger A-D11 route: a NEW top-level struct, zero change to `TestReport`.** That route carries zero risk, and this measurement only establishes that the weaker option was *available*, not that it was needed. `crates/mcp-tester/src/post_deploy_report.rs` is the in-repo shape to copy.

**⚠ ADDENDUM — a SECOND `TestResult` struct literal exists that A-D11 did not name.** A-D11 cites one (`apps.rs:874-880`). There are **two**:

| Location | Compiled by `cargo build -p cargo-pmcp`? |
|---|---|
| `cargo-pmcp/src/commands/test/apps.rs:875` | yes |
| `cargo-pmcp/src/commands/test/check.rs:522` (inside `#[cfg(test)] mod tests`) | **NO** |

Both are exhaustive positional literals, so **adding a field to `TestResult` breaks both**. The plan's acceptance criterion — `cargo build -p cargo-pmcp` — **structurally cannot see the second one**, because `#[cfg(test)]` modules are not compiled by `cargo build`. This plan therefore recorded `cargo check -p cargo-pmcp --tests` as a companion baseline (Rule 2: the stated gate was insufficient for the surface it claims to guard). **117-11 must run BOTH.** The constraint is unchanged in force — do NOT add a field to `TestResult` — but the gate that proves it is now complete.

### A-CI: CI is not a consumer of the report shape

No CI job runs `mcp-tester` against a live server. `.github/workflows/mcp-tester-validation.yml:59-62` stubs the binary to `echo` (`echo "MCP_TESTER_BIN=echo" >> $GITHUB_ENV`), and that workflow is absent from `ci.yml`'s `gate.needs` at `.github/workflows/ci.yml:443` (`needs: [test, quality-gate, purity-check, pmcp-agent-targets, wasm32-purity]`). The strict consumers are the six in-repo library linkers — i.e. **the Rust compiler**, not a JSON parser.

## Negative Controls (executed, recorded, reverted)

### Control 1 — json byte assertion (Task 1)

Renamed one fixture `TestResult` from `"initialize"` to `"initialize-NEGATIVE-CONTROL"`. The byte assertion failed as designed:

```
thread 'json_single_run_output_is_byte_pinned' panicked at crates/mcp-tester/tests/report_compat.rs:249:5:
assertion `left == right` failed: ADDITIVITY BREAK: the single-run `--format json` output of
mcp-tester changed. A-D11 forbids this — plan 117-11's `--dual-run` work must be ADDITIVE. Do NOT
re-record this golden. Either move the change behind the opt-in `--dual-run` path, or put the new
data in a new top-level struct the way `crates/mcp-tester/src/post_deploy_report.rs` does. Raw
output was:
{
  "tests": [
    {
      "name": "initialize-NEGATIVE-CONTROL",
```

Reverted; re-ran to `3 passed; 0 failed`.

### Control 2 — multiset multiplicity (Task 2)

Duplicated one **already-rendered** line verbatim and, in the same patched run, computed the distinct-line SET comparison as evidence:

```
---- pretty_multi_category_line_multiset_is_pinned stdout ----
SET-COMPARISON-WOULD-PASS: true

thread 'pretty_multi_category_line_multiset_is_pinned' panicked at
crates/mcp-tester/tests/report_compat.rs:696:5:
ADDITIVITY BREAK: the `--format pretty` line multiset changed.
  line "✓ ping                                              round trip ok": expected 1, got 2
Do NOT re-record this golden — make the change `--dual-run`-only, or put it in a new top-level
struct the way `crates/mcp-tester/src/post_deploy_report.rs` does.
```

The `SET-COMPARISON-WOULD-PASS: true` line is the point: a set comparison of the *same* input returns EQUAL, so the multiset is doing real work. Reverted; re-ran to `7 passed; 0 failed`.

**Why a rendered line and not a fixture entry:** duplicating a fixture `TestResult` also moves the summary counters (`total` 6→7, `passed` 3→4), which changes other output lines — a set comparison would then catch it too, and the control would prove nothing about multiplicity. Duplicating a rendered line leaves the distinct-line set untouched, which is exactly the regression class a set misses.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — Missing critical coverage] `cargo build -p cargo-pmcp` cannot see the second `TestResult` literal**

- **Found during:** Task 2, A2 re-measurement
- **Issue:** The plan's acceptance criterion and threat mitigation for T-117-07 rely on `cargo build -p cargo-pmcp` catching a `TestResult` field addition. The second literal (`check.rs:522`) is inside `#[cfg(test)] mod tests`, which `cargo build` does not compile — so the stated gate guards only one of the two surfaces.
- **Fix:** Recorded `cargo check -p cargo-pmcp --tests` (exit 0) as a companion baseline and flagged it above as a requirement for 117-11.
- **Files modified:** none (measurement + summary record only)
- **Commit:** `6d829e2d`

**2. [Rule 1 — Bug] Category-block parser stopped at the header's trailing blank line**

- **Found during:** Task 2, first run
- **Issue:** `print_pretty` writes a blank line *between* the category header and its test lines (`report.rs:275`). A naive `take_while(non-empty)` starting at `header + 1` therefore yielded an empty block and the ordering assertion failed vacuously-inverted (`observed block was []`).
- **Fix:** Added `skip_while(is_empty)` before the `take_while`.
- **Files modified:** `crates/mcp-tester/tests/report_compat.rs`
- **Commit:** `6d829e2d`

**3. [Rule 1 — Bug] Generated multiset const collapsed internal whitespace**

- **Found during:** Task 2, golden generation
- **Issue:** The first generator used `awk` field-rebuild (`$1=""`) to strip the `uniq -c` count, which rebuilds `$0` with `OFS` and collapses the pretty printer's 40-column name padding. The resulting const would have compared whitespace-normalized lines, silently defeating non-determinism source 3 (the conditional duration column changes line WIDTH).
- **Fix:** Regenerated with `sed -E 's/^ *([0-9]+) (.*)$/    ("\2", \1),/'`, which preserves the line bytes exactly. Verified in the emitted const.
- **Files modified:** `crates/mcp-tester/tests/report_compat.rs`
- **Commit:** `6d829e2d`

### Scope adjustments

**4. Fixture strengthened between Task 1 and Task 2's design, before Task 1 was committed**

The initial 4-test fixture put exactly one test in each category, which would have made Task 2's intra-block ordering assertion a tautology over one-element blocks. The fixture was widened to 6 tests / 3 categories / 2 per category and **interleaved** before the Task 1 golden was recorded, so the json golden never had to be re-recorded for a Task 2 reason.

**5. Plan line citations had drifted from HEAD; both are recorded rather than silently corrected**

The plan cites three `report.rs` ranges. Measured at HEAD: `:262-282` (HashMap grouping) is accurate; `:257-259`/`:294-298` (the `colored` conditional) are actually `:259-260` and `:298-303`; `:305-309` (claimed to be the duration column) is actually the adjacent **name-truncation** branch — the duration conditional is `:313-318`. The in-file header comment records the plan's cited range AND the measured one for each, so the acceptance criterion's grep tokens are present and a future reader is not sent to the wrong lines.

### Authentication gates

None.

## Verification

| Check | Result |
|---|---|
| `cargo test -p mcp-tester --test report_compat` | **7 passed; 0 failed** (floor was 6) |
| Same, repeated 8× under randomized hasher seeds | 7 passed / 0 failed on every run |
| `cargo build -p cargo-pmcp` | **exit 0** — the recorded pre-change baseline |
| `cargo check -p cargo-pmcp --tests` | **exit 0** — companion baseline (see Deviation 1) |
| `git diff --stat crates/mcp-tester/src/ crates/mcp-tester/Cargo.toml` | **empty** — no `mcp-tester` source or manifest touched |
| `grep -c 'print_to_writer' …/report_compat.rs` | 2 (floor 1) |
| `grep -c 'BTreeSet' …/report_compat.rs` | **0** |
| `grep -c 'width-preserving'` / `'FAILURE MODE'` / `'post_deploy_report'` / `'do NOT re-record'` / `'tty detection'` | 2 / 3 / 3 / 1 / 2 |
| `grep -cE 'TODO\|FIXME\|XXX' …/report_compat.rs` | **0** |
| `cargo fmt -p mcp-tester -- --check` | clean |
| `make quality-gate` | **exit 0** |

## Known Stubs

None. No stub, placeholder, or unwired data path was introduced — every golden in the file is a captured value asserted by a passing test.

## Threat Flags

None. This plan adds one integration-test file and touches no source, so it introduces no network endpoint, auth path, file-access pattern, or schema change at a trust boundary.

## Notes for 117-11

1. Take the **new-top-level-struct** route. `TestReport` may legally gain an optional field (A2 confirmed), but the struct route has zero risk and `post_deploy_report.rs` is the shape to copy.
2. Run **both** `cargo build -p cargo-pmcp` **and** `cargo check -p cargo-pmcp --tests`. The second `TestResult` literal at `check.rs:522` is invisible to the first.
3. Do NOT add a field to `TestResult`, a variant to `TestCategory` or `TestStatus`, or change `ServerTester::new`'s arity. All four are hard compile breaks in `cargo-pmcp`.
4. If a golden here goes red, that is the signal working — not a stale fixture.

## Self-Check: PASSED

- `crates/mcp-tester/tests/report_compat.rs` — FOUND (771 lines)
- `.planning/phases/117-agents-tester-v1-severability/117-03-SUMMARY.md` — FOUND
- Commit `39bda1d2` (`test(117-03): pin mcp-tester --format json single-run bytes at 0.7.0`) — FOUND
- Commit `6d829e2d` (`test(117-03): pin mcp-tester --format pretty contract and re-measure A2`) — FOUND
