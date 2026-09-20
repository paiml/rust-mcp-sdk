---
phase: 117-agents-tester-v1-severability
plan: 08
subsystem: mcp-tester
tags: [era-diff, baseline, spec-artifact, tripwire, fuzz, CLNT-04, D-06]
requires:
  - "117-03 (report_compat goldens — the A-D11 additivity proof this plan must keep green)"
provides:
  - "crates/mcp-tester/baselines/era-deltas.yaml — the checked-in v1-vs-v2 expected-difference baseline (14 cited entries)"
  - "mcp_tester::era_diff — EraDelta / EraBaseline + parse_baseline / load_baseline / load_default_baseline / default_baseline_path"
  - "EraDelta::observation_id — the stable machine-facing join key plan 117-11 diffs on"
  - "crates/mcp-tester/tests/era_baseline.rs — schema + non-vacuity gate (MINIMUM_DELTAS = 14)"
  - "fuzz target era_deltas_parser — CLAUDE.md ALWAYS fuzz requirement discharged"
affects:
  - "117-11 (dual-run comparison joins on observation_id; adds one probe per entry)"
  - "Phase 118 conformance work (the baseline is a direct input)"
tech-stack:
  added: []
  patterns:
    - "New top-level module, never a field on TestResult (post_deploy_report.rs precedent, A-D11)"
    - "Checked-in YAML data file loaded via serde_yaml (scenario.rs::from_yaml_file idiom)"
    - "Named-floor non-vacuity tripwire with FAILURE MODE / WHAT TO DO messages (phase115_contract_bindings.rs idiom)"
    - "Validation as a PARSER CONTRACT so the fuzz invariant matches what the parser guarantees"
key-files:
  created:
    - crates/mcp-tester/baselines/era-deltas.yaml
    - crates/mcp-tester/src/era_diff.rs
    - crates/mcp-tester/tests/era_baseline.rs
    - fuzz/fuzz_targets/era_deltas_parser.rs
  modified:
    - crates/mcp-tester/src/lib.rs
    - fuzz/Cargo.toml
decisions:
  - "D-117-08-FORMAT: the baseline is YAML, not TOML — serde_yaml is already a dependency of mcp-tester and already loads checked-in data; adding `toml` would be a new dependency on a published 0.7.0 crate for zero gain (comments are the only property the 'reviewable as a spec artifact' requirement needs, and YAML has them)"
  - "D-117-08-CONTRACT: non-empty unique `id` and `observation_id` are enforced INSIDE parse_baseline, not only in a test, so the fuzz target's Ok-path assertions are exactly the parser's documented rejections"
  - "D-117-08-VACUITY: an EMPTY `deltas:` list is deliberately NOT a parser rejection — the non-vacuity floor lives in tests/era_baseline.rs where the failure message can explain it (T-117-26)"
metrics:
  duration_minutes: 82
  completed: 2026-08-08
  tasks: 3
  commits: 3
  files_created: 4
  files_modified: 2
  lines_added: 1048
---

# Phase 117 Plan 08: Era-Delta Baseline Summary

The expected-difference baseline between MCP 2025-11-25 and 2026-07-28 now exists as a
reviewable, citation-bearing YAML spec artifact with a contract-enforcing loader, a named-floor
schema gate whose three failure modes were executed, and a fuzz target that asserts exactly what
the parser guarantees.

## What was built

| Artifact | Lines | What it is |
|---|---|---|
| `crates/mcp-tester/baselines/era-deltas.yaml` | 241 | 14 cited, comment-carrying entries. The written statement of what "dual-version" means for this SDK. |
| `crates/mcp-tester/src/era_diff.rs` | 382 | NEW top-level module: `EraDelta`, `EraBaseline`, `parse_baseline`, `load_baseline`, `load_default_baseline`, `default_baseline_path`. 7 unit tests + 2 property tests. |
| `crates/mcp-tester/tests/era_baseline.rs` | 333 | 7-test schema gate: id uniqueness, observation-id presence/uniqueness/shape, citation floor, non-vacuity floor, protocol-version sync, provisional ownership, parser totality. |
| `fuzz/fuzz_targets/era_deltas_parser.rs` | 74 | CLAUDE.md ALWAYS fuzz target. |

## The format decision: YAML, not TOML — and why

RESEARCH § Q5.3 recommended a TOML table. **That recommendation was overridden**, and the plan
directed the override:

- `serde_yaml = "0.9"` is **already** a dependency of `mcp-tester` (`crates/mcp-tester/Cargo.toml:26`)
  and **already** loads checked-in data files (`src/scenario.rs:232-238`, `TestScenario::from_yaml_file`).
  This crate's own scenarios ship as `scenarios/*.yaml`.
- Adding `toml` would be a **NEW dependency on an already-published 0.7.0 crate**, which the
  CLAUDE.md package discipline requires justifying.
- It would buy nothing. Comment support is the only property the "legible enough to review as a
  spec artifact" requirement actually needs, and YAML has it.

`git diff crates/mcp-tester/Cargo.toml` is **EMPTY** — verified. Zero new packages were installed
anywhere in this plan (T-117-SC discharged); `fuzz/Cargo.toml` gained only a `[[bin]]` block.

The decision is stated in the baseline file's own header (`§ WHY YAML AND NOT TOML`) so a future
reader does not have to find this summary.

## The 14 entries: `id` → `observation_id`, and whether each citation resolved

`observation_id` is the STABLE, MACHINE-FACING join key. It exists because `TestResult`
(`crates/mcp-tester/src/report.rs:73-81`) carries only `{name, category, status, duration, error,
details}` — no header, no session id, no result-envelope key, no HTTP status — so plan 117-11's
comparison could not observe most of these entries if it keyed on human-facing test names.

**Every source citation in RESEARCH § Q5.2 was opened at the cited line before being written into
the baseline.** Nine resolved verbatim; five had drifted or were wrong and were CORRECTED in the
file rather than copied.

| id | `observation_id` | prov. | Citation check |
|---|---|---|---|
| ERA-01 | `method.initialize` | | ⚠ **CORRECTED.** `REQUIREMENTS.md:913` is **CLNT-03**, not the initialize goal. The requirement that says "no `initialize`" is **CLNT-01 at `:911`** — baseline cites `:911`. Rust half resolved: `v2_synthetic_initialize_result` at `src/client/mod.rs:726-741` says "v2 removed `initialize`, so no byte of this came from the server". |
| ERA-02 | `method.server_discover` | | ✅ RESOLVED. `client/mod.rs:887` — "a v1 server answers `-32601`". `core.rs:1180-1187` — `project_capabilities_for_v2`. |
| ERA-03 | `header.mcp_session_id` | | ⚠ **CORRECTED (drift).** Neither cited range resolved. The rule predicate is `sessions_active_for` at `:455-460`; the spec quote *"ignore it, and do not mint or echo session IDs"* is at **`:1809-1811`** (research said `:1766-1772`). Both corrected in the file. |
| ERA-04 | `header.mcp_method_and_name` | | ✅ RESOLVED EXACTLY. `http_constants.rs:17-31` — `MCP_METHOD` at `:23`, `MCP_NAME` at `:31`, both doc-tagged "(VERS-05, v2 `2026-07-28`)". |
| ERA-05 | `header.last_event_id` | | ⚠ **PARTIAL.** `REQUIREMENTS.md:992` resolved EXACTLY ("v2 removes `Last-Event-ID`; retrofitting fights the stateless model"). The `shs:476-498` half did **not** — that range is `active_session_generator`. The resumability era gate is `:514-535` (carrying the verbatim spec quote) plus `resumability_active_for` at `:561-566`. Corrected. |
| ERA-06 | `http.verb.get_delete` | | ⚠ **CORRECTED (drift).** `v2_method_not_allowed` is at **`:1649-1658`** and `v2_verb_rejection` at **`:1664-1677`**, not `:1610-1640`. Symbols exist and say what the research claims. |
| ERA-07 | `result.result_type` | ✅ | ✅ RESOLVED EXACTLY. `.planning/ROADMAP.md:2241` is Phase 112 success criterion 5, verbatim `resultType` (`complete`/`input_required`/`task`). |
| ERA-08 | `result.server_info` | ✅ | ✅ RESOLVED EXACTLY. `.planning/ROADMAP.md:2237` is criterion 1, "v2 results carry `serverInfo`". |
| ERA-09 | `method.tasks_list` | ✅ | ✅ RESOLVED EXACTLY. `114-CONTEXT.md:15` and `:203` (D-15: "on v2, `tasks/get` inlines the result; `tasks/result` and `tasks/list` answer …"). |
| ERA-10 | `capability.tasks_location` | ✅ | ✅ RESOLVED EXACTLY. `114-CONTEXT.md:52-60` (D-02) and `:62-70` (D-03); `core.rs:1156` (`EXPERIMENTAL_TASKS_KEY`) and `:1180-1187` (`project_capabilities_for_v2`). |
| ERA-11 | `result.cache_scope` | | ✅ RESOLVED EXACTLY. `115-CONTEXT.md:74-87` (D-07 AMENDED) and `:88-98` (D-08). Confirms REQUIRED-not-optional and the `ttlMs: 0` / `cacheScope: "private"` safe default. |
| ERA-12 | `method.resources_subscribe` | | ✅ RESOLVED EXACTLY, all three. `REQUIREMENTS.md:915` **is** CLNT-05; `client/mod.rs:697-705` is `reject_if_retired_on_v2`; `error/mod.rs:126-131` is `RETIRED_ON_V2_MARKER`. |
| ERA-13 | `method.subscriptions_listen` | | ✅ RESOLVED. `113-CONTEXT.md:41` (D-13) confirms the capability-gated SKIPPED-conformant rule verbatim. The `shs` half drifted slightly; corrected to `:3322-3329` (`assemble_subscriptions_listen`). |
| ERA-14 | `http.status.error_code_mapping` | | ⚠ **CORRECTED (drift).** `v2_status_for_code` is at **`:729`** (research said `:690-722`) and `status_mapping_is_era_gated_so_v1_is_untouched` at **`:4988`** (research said `:4949`). Both symbols exist. |

All 14 `observation_id` values are unique (`grep 'observation_id:' … | sort | uniq -d` is empty),
lowercase, dot-separated, and carry at least one `.` — gated by
`every_delta_observation_id_is_present_and_unique`.

**4 entries are `provisional: true`** (floor was 2), each with an adjacent comment naming its owner
and a `note` naming the phase:

- ERA-07, ERA-08 — **Phase 112** owns the v2 result envelope; VERS-07 values come only from the
  final `schema.json`, so these rows can move.
- ERA-09, ERA-10 — **Phase 114**, whose task surface is PROVISIONAL per 117 D-09.

## `parse_baseline`'s documented rejections vs. the fuzz target's assertions

The plan's central correctness requirement: **the fuzz target must assert only what the parser
actually guarantees**, or valid-YAML-with-`id: ""` would crash the fuzzer on well-formed input.
Validation was therefore put INTO the parser.

| `parse_baseline` doc comment enumerates as a rejection | `era_deltas_parser.rs` asserts on the `Ok` path |
|---|---|
| 1. text is not valid YAML for the schema → `Err` | (no assertion — the `Err` path returns early) |
| 2. some delta's `id` is empty after trimming → `Err` | `!delta.id.trim().is_empty()` |
| 3. some delta's `observation_id` is empty after trimming → `Err` | `!delta.observation_id.trim().is_empty()` |
| 4. two deltas share an `id` → `Err` | `ids.insert(delta.id)` returns `true` |
| 4. two deltas share an `observation_id` → `Err` | `observation_ids.insert(…)` returns `true` |
| — | `baseline.observation_ids().len() == baseline.deltas.len()` (accessor agrees with the collection it projects — a total function over an already-validated value, cannot fail on accepted input) |

**The two sets agree.** Every invariant the target asserts is one `parse_baseline` documents
rejecting the negation of; no assertion was dropped, and no assertion was added that the parser
does not enforce. The approach taken was the plan's option A: **move the validation into
`parse_baseline`**.

Explicitly NOT enforced by the parser, and documented as such in its doc comment: the lexical
shape of an `observation_id`, the length of a `source`, and whether a provisional entry names its
owner. Those are baseline-CONTENT rules gated by `tests/era_baseline.rs` against the checked-in
file, not properties of arbitrary input.

## Fuzz run — exact command and result

`cargo fuzz build era_deltas_parser` on the **default stable toolchain FAILS**:

```
error: the option `Z` is only accepted on the nightly compiler
help: consider switching to a nightly toolchain: `rustup default nightly`
```

This is an environment fact, not a defect in the target — `cargo-fuzz` requires nightly for
`-Zsanitizer=address`. `nightly-aarch64-apple-darwin` is installed. Recorded so a future plan does
not misread it as a build break.

```
$ cargo +nightly fuzz build era_deltas_parser
    Finished `release` profile [optimized + debuginfo] target(s) in 2m 16s
  exit 0

$ cargo +nightly fuzz run era_deltas_parser -- -runs=20000
#20000  DONE   cov: 2004 ft: 3885 corp: 462/1908b lim: 8 exec/s: 20000 rss: 291Mb
Done 20000 runs in 1 second(s)
```

`fuzz/artifacts/era_deltas_parser/` is **empty** — no crash, no timeout, no OOM. 462 corpus entries
were retained. T-117-27 discharged.

`fuzz/Cargo.toml` gained exactly one `[[bin]]` block (`test = false`, `doc = false`, `bench = false`)
and **zero** `[dependencies]` entries — `mcp-tester` was already a path dependency at `:53-54`.

## Negative controls — EXECUTED, output verbatim, then reverted

### NC1 — three entries deleted (ERA-12/13/14), floor must fire

```
running 7 tests
test the_baseline_parse_is_not_vacuous ... FAILED
[6 others ok]

---- the_baseline_parse_is_not_vacuous stdout ----
FAILURE MODE: parsed 11 delta(s) from baselines/era-deltas.yaml, below the 14 floor. A reader that
silently reads nothing makes every era diff built on this file pass over an empty set, and every
other test in this file pass vacuously.
WHAT TO DO: fix the reader or restore the file; do not lower the floor.

test result: FAILED. 6 passed; 1 failed
```

### NC2 — ERA-02's `id` duplicated to `ERA-01`

```
---- every_delta_id_is_unique stdout ----
FAILURE MODE: the checked-in baseline at …/baselines/era-deltas.yaml did not load: Failed to parse
era-delta baseline: …: era-delta baseline: duplicate `id` `ERA-01`
WHAT TO DO: fix the reader or restore the file; do not delete this gate.

test result: FAILED. 1 passed; 6 failed
```

### NC3 — ERA-02's `observation_id` duplicated to `method.initialize`

```
---- every_delta_observation_id_is_present_and_unique stdout ----
FAILURE MODE: the checked-in baseline at …/baselines/era-deltas.yaml did not load: Failed to parse
era-delta baseline: …: era-delta baseline: duplicate `observation_id` `method.initialize` (on entry
`ERA-02`)
WHAT TO DO: fix the reader or restore the file; do not delete this gate.

test result: FAILED. 1 passed; 6 failed
```

**Honest note on the MECHANISM of NC2/NC3.** The plan's criteria say
`every_delta_id_is_unique` / `every_delta_observation_id_is_present_and_unique` "MUST fail naming
the duplicate", and both did — **but via the loader gate, not via their own assertion body.**
Because Task 1 made uniqueness a PARSER CONTRACT, `load_baseline` refuses the mutated file before
any test's assertion runs, so all six loading tests fail together with the parser's message. The
duplicate is named in every one of them. This is a strictly stronger outcome than the criterion
asked for (the defect cannot even be loaded), but it is a different mechanism than the criterion's
wording implies, and it is recorded rather than glossed. The tests' own uniqueness assertions
remain as defence in depth for any future caller that bypasses `parse_baseline`.

All three controls were reverted (`git checkout crates/mcp-tester/baselines/era-deltas.yaml`) and
the suite re-run to **7 passed; 0 failed**.

## Verification — every plan criterion, measured

| Check | Result |
|---|---|
| `cargo test -p mcp-tester --lib era_diff` | **9 passed** (7 unit + 2 property) |
| `cargo test -p mcp-tester --test era_baseline` | **7 passed; 0 failed** |
| `cargo test -p mcp-tester --test report_compat` | **7 passed** — 117-03's goldens still green, single-run output unchanged |
| `cargo build -p mcp-tester` | exit **0** |
| `cargo build -p cargo-pmcp` | exit **0** |
| `cargo check -p cargo-pmcp --tests` | exit **0** (the companion gate for the second `TestResult` literal, carried forward from 117-03) |
| `cargo +nightly fuzz build era_deltas_parser` | exit **0** |
| `git diff crates/mcp-tester/Cargo.toml` | **EMPTY** — no `toml` dependency |
| `git diff crates/mcp-tester/src/report.rs` | **EMPTY** — `TestResult`/`TestCategory`/`TestStatus` untouched (T-117-29) |
| `grep -cE '^[[:space:]]*- id:' era-deltas.yaml` | **14** |
| `grep -c 'source:' era-deltas.yaml` | **14** (= entry count) |
| `grep -c 'observation_id:' era-deltas.yaml` | **14**; `sort \| uniq -d` **empty** |
| `grep -c 'provisional: true' era-deltas.yaml` | **4** (floor 2) |
| `grep -c 'FAILURE MODE' tests/era_baseline.rs` | **14** (floor 3) |
| `grep -c 'MINIMUM_DELTAS' tests/era_baseline.rs` | **5** (floor 2) |
| `grep -c '"/Users\|"/home' tests/era_baseline.rs` | **0** — path derived from `CARGO_MANIFEST_DIR` |
| `grep -c '2026-07-28' tests/era_baseline.rs` | **0** — versions imported from `pmcp::LATEST_PROTOCOL_VERSION` and `pmcp::types::protocol::version::PROTOCOL_VERSION_2026_07_28` |
| `grep -c 'TODO\|FIXME\|XXX' tests/era_baseline.rs` | **0** |
| `era_diff.rs` module doc contains `post_deploy_report` / `cargo-pmcp` | **2** / **4** occurrences |
| `cargo clippy -p mcp-tester --all-targets` | 5 warnings, **all pre-existing** (`scenario_executor.rs:653,671`; `examples/render_ui.rs:88`; 2 in `pmcp` `src/server/auth/jwt*.rs`). **Zero** from this plan's files. |
| `make quality-gate` | exit **0** — "✅ ALL TOYOTA WAY QUALITY CHECKS PASSED" |

### ⚠ Gate-scope finding: `make quality-gate` does NOT run this plan's tests

Measured from the gate's own 9239-line transcript: `era_baseline` and `era_diff` appear **0 times**.
The gate's `test-unit` population is **1880**, byte-identical to the anchor recorded across Phases
115–116. `make lint` and `make test-all` are scoped to the root `pmcp` package, so **no `mcp-tester`
test is inside the gate at all.**

This EXTENDS the recorded `LIM-116-10` gate-scope hole from "six oauth-gated core binaries report
0 passed" to "the whole `mcp-tester` crate is outside the gate". Every check in the table above was
therefore run **directly**, not inferred from a green gate. Logged to `deferred-items.md`; not
fixed here (fixing it means editing the `Makefile`, which no task in this plan owns, and the
LIM-116-10 pairing requirement — clear the failures first, then widen the gate — still stands).

## Deviations from Plan

### 1. [Rule 1 — Bug in this plan's own new test] `the_parser_rejects_garbage_without_panicking` had a false premise

- **Found during:** Task 2, first run (a genuine RED).
- **Issue:** One "garbage" input was
  `schema_version: 1\nv1_protocol: 1\nv2_protocol: 2\ndeltas: []\n`, asserted to be `Err`.
  `parse_baseline` **accepted** it. Two measured facts explain why, and neither is a parser bug:
  (a) `serde_yaml` 0.9 **coerces a bare YAML scalar into a `String` field**, so `v1_protocol: 1`
  deserializes as `"1"`; (b) an **empty `deltas:` list is legitimately accepted** — non-vacuity is
  the TEST's job, per the plan's own threat register (T-117-26), which places the floor in
  `MINIMUM_DELTAS` so its failure message can explain the remedy.
- **Fix:** Fixed the TEST's premise, not the parser. The input was replaced with two inputs that
  exercise real rejections — a delta missing the required `observation_id`, and a delta whose
  `observation_id` is present but empty. Tightening the parser to reject an empty list was
  **rejected**: it would move the floor out of the one place whose failure message explains it, and
  would put the parser's contract out of step with the fuzz target's assertions.
- **Recorded:** the measured boundary is now a `//! # Measured boundary of parse_baseline` section
  in `tests/era_baseline.rs`, so the next reader does not re-litigate it.
- **Files:** `crates/mcp-tester/tests/era_baseline.rs`. **Commit:** `a909de92`.

### 2. [Rule 2 — CLAUDE.md ALWAYS requirement] Property tests added

- **Trigger:** CLAUDE.md mandates FUZZ + PROPERTY + UNIT + EXAMPLE for every new feature. The plan
  specified fuzz and unit but not property.
- **Fix:** Two `proptest` cases added inside `era_diff.rs`'s existing `#[cfg(test)]` module — no new
  file, no new dependency (`proptest = "1"` is already a dev-dependency at
  `crates/mcp-tester/Cargo.toml:44`): `parse_baseline` is TOTAL over arbitrary text, and every
  ACCEPTED baseline satisfies the documented uniqueness/non-emptiness contract.
- **Files:** `crates/mcp-tester/src/era_diff.rs`. **Commit:** `a80f770e`.

### 3. [Scope] The EXAMPLE half of the CLAUDE.md ALWAYS rule is deferred, deliberately

The baseline has no user-facing runtime surface yet — the `--dual-run` CLI flag that consumes it is
owned by **plan 117-11**. An example now would demonstrate a loader with no consumer, and adding a
file outside this plan's `files_modified` is scope creep. **117-11 owns the example**, which will
exercise the baseline through the dual-run path it was written for. Logged to `deferred-items.md`.

### 4. [Citations] Five of fourteen research citations were corrected, not copied

Documented per-entry in the table above. The plan required opening each cited line; doing so found
one **wrong requirement id** (ERA-01 cited CLNT-03 where it meant CLNT-01) and four **line drifts**
in `streamable_http_server.rs` / a wrong function range. All were corrected in the baseline file
against the symbols as they exist at this commit. No entry was written on an unverified citation.

## Threat Flags

None. This plan adds no network endpoint, no auth path, no file-access pattern outside
`CARGO_MANIFEST_DIR`, and no schema at a trust boundary. The one new parser is fuzzed
(T-117-27) and its data file is floor-gated (T-117-26).

## Known Stubs

None. `era_diff` ships a complete, tested data model and reader. The absent piece —
`DualRunReport` and the comparison logic — is **not a stub**: it is explicitly out of scope
("plan 117-11 owns that"), is named as such in the module's `# Scope` doc section, and its join key
(`observation_id`) is delivered and gated here so 117-11 can build against it.

## Handoff to 117-11

1. **Join on `observation_id`, never on `TestResult::name`.** `EraBaseline::find_by_observation_id`
   and `EraBaseline::observation_ids` exist for this. The reason is in `era_diff.rs`'s field doc.
2. **Add one probe per `observation_id`** plus the two-direction coverage test between the probe
   registry and the baseline. All 14 keys are listed in the table above.
3. **The A-D11 rule is absolute and now has two guards**, both green at this commit:
   `cargo build -p cargo-pmcp` (catches `apps.rs:875`) and `cargo check -p cargo-pmcp --tests`
   (catches `check.rs:522`, inside `#[cfg(test)]`, which `cargo build` never sees).
4. **`make quality-gate` will not catch a regression in this plan's files** — see the gate-scope
   finding above. Run `cargo test -p mcp-tester` explicitly.

## Commits

| Commit | Type | Description |
|---|---|---|
| `a80f770e` | feat | era-delta baseline + loader (`era-deltas.yaml`, `era_diff.rs`, `lib.rs` barrel) |
| `a909de92` | test | schema + non-vacuity tripwire (`tests/era_baseline.rs`), 3 negative controls executed |
| `95acfa02` | test | fuzz target + `[[bin]]` registration |

## Self-Check: PASSED

All 4 created files exist on disk; all 3 commit hashes resolve in `git log --all`.
