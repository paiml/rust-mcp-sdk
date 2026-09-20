---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 09
subsystem: testing
tags: [fuzzing, libfuzzer, cargo-fuzz, proptest, json-schema, draft-2020-12, caching, mcp-2026-07-28, examples]

# Dependency graph
requires:
  - phase: 115-03
    provides: "the era-branched validation path — `normalize_schema_dialect`, `compile_2020_12` and the UNCACHED `compile_for_era` the fuzz seam drives"
  - phase: 115-04
    provides: "`CallToolResult::structured_value`, the non-object structured-output constructor the property test and the example exercise"
  - phase: 115-05
    provides: "`CacheScope`, `DEFAULT_TTL_MS` and the cfg-free `project_caching_hints` the two closed-union properties and the example demonstrate"
  - phase: 115-06
    provides: "the native chokepoint wiring that makes the example's v2/v1 contrast observable on the wire"
provides:
  - "`output_validation::fuzz_support` — a `fuzzing`+`validation`-gated seam returning a THREE-state `SchemaVerdict`, using the uncached compile path, invisible on `default` and `full`"
  - "`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` — three TRUE invariants over the era branch (totality, normalization idempotence-and-surgical-scope, dialect-neutral era AGREEMENT)"
  - "`fuzz/corpus/fuzz_schema_draft_pin/` — 11 committed, hand-written, length-prefixed seeds plus a README, the first committed fuzz corpus in this repo"
  - "four new property tests: the closed `CacheScope` union over arbitrary strings, `$schema` normalization over arbitrary schemas, `structuredContent` shape preservation"
  - "`examples/s52_v2_caching_hints.rs` — the runnable ALWAYS example for SCHM-02 + SCHM-03, asserting all four claims"
affects: [115-10, any-phase-touching-output_validation, any-phase-adding-a-fuzz-seam]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Three-state fuzz verdict: a seam that must express 'skip this comparison' cannot return `(bool, bool)`"
    - "Length-prefixed fuzz input layout, so a seed corpus is hand-writable and the target reaches its real code path"
    - "Neutrality predicate instead of a monotonicity assertion, when the property is only true on a keyword subset"

key-files:
  created:
    - fuzz/fuzz_targets/fuzz_schema_draft_pin.rs
    - fuzz/corpus/fuzz_schema_draft_pin/ (11 seeds + README.md)
    - examples/s52_v2_caching_hints.rs
  modified:
    - src/server/output_validation.rs
    - src/server/mod.rs
    - src/types/caching.rs
    - tests/property_tests.rs
    - fuzz/Cargo.toml
    - fuzz/.gitignore
    - Cargo.toml

key-decisions:
  - "Invariant 3 is an EQUALITY over dialect-neutral schemas, never a cross-dialect monotonicity claim — monotonicity is false in BOTH directions and the repo's own tests are the counterexamples"
  - "The divergence example is `contentEncoding`, not `dependencies` — the plan inherited D-115-03-C's error and it is corrected here"
  - "Neutrality also restricts the ROOT `$schema` to absent/draft-07/2020-12 and forbids a nested `$schema`, because draft-04 changes the meaning of the allowlisted keyword `type`"
  - "The seam calls `compile_for_era`, never `cached_validator`, so fuzzing cannot grow the process-global validator cache without bound"
  - "The `Some(Value::Null)` round-trip asymmetry (D-115-04-A) is asserted as MEASURED rather than fixed — `src/types/tools.rs` is outside this plan and the fix changes every client's parse"
  - "`ALL_SCOPES` is `#[cfg(test)]`, matching `types::tasks::ALL_STATUSES`, so the phase's `cargo public-api diff` stays empty"

patterns-established:
  - "Fuzz seam convention: third copy of the `request_state` / `task_dispatch` twin-cfg module widening, now with an explicit three-state verdict type"
  - "Committed seed corpora: `fuzz/.gitignore` re-includes `README.md` + `[0-9][0-9]_*` under one target's corpus dir, keeping libFuzzer's runtime units ignored"
  - "Negative controls on both a fuzz invariant and an example assertion, with the crash output transcribed"

requirements-completed: [SCHM-01, SCHM-02, SCHM-03]

# Metrics
duration: 2h36m
completed: 2026-08-01
---

# Phase 115 Plan 09: ALWAYS Requirements (Fuzz, Property, Example) Summary

**A `fuzzing`-gated three-state seam on the era-branched `outputSchema` validator, fuzzed at 10.3k exec/s over 11 committed hand-written seeds with three invariants that are actually TRUE, plus four new property tests and `s52_v2_caching_hints` — a runnable example that asserts the handler-set posture, the safe default, the v1 era gate and non-object `structuredContent`.**

## Performance

- **Duration:** 2h 36m (roughly 1h 50m of it in gate/fuzz/doctest wall time)
- **Started:** 2026-08-01T12:33:00Z
- **Completed:** 2026-08-01T15:09:00Z
- **Tasks:** 4
- **Files modified:** 7 modified, 13 created

## Accomplishments

- **A fuzz target whose invariants are true.** The pre-review version asserted cross-dialect monotonicity, which this phase's own design makes false. This one asserts totality, normalization idempotence-and-surgical-scope, and era AGREEMENT restricted to dialect-neutral schemas — and a negative control proves invariant 3 actually fires on the vacuous-pin regression.
- **The first committed fuzz corpus in this repo.** `fuzz/corpus` was gitignored wholesale; the ignore is now narrowed so this target's 11 named seeds and its README are tracked while libFuzzer's runtime units stay out. Replaying the corpus is a command that can fail, and it does fail under the negative control.
- **10,315 exec/s over 629,244 runs in 61 s, zero crash artifacts** — with the committed seeds reaching schema COMPILATION rather than the target degenerating into a JSON-parser fuzz (cov 10,586 / ft 25,778 versus cov 8,394 on the seeds alone).
- **`cargo public-api diff 0936f46e..HEAD` reports "(none)" in all three sections** — the seam adds nothing to the shipped surface, measured precisely against this plan's own boundary rather than against a release baseline.
- **A runnable example that teaches the security semantics.** `s52_v2_caching_hints` leads with what `cacheScope: "public"` authorizes — serving one caller's body across authorization contexts — and shows the v1 client completing a real `initialize` handshake with a session while the v2 client sends neither.

## Task Commits

1. **Task 1: fuzzing seam + fuzz target + seed corpus** — `ddfb450b` (test)
2. **Task 2: property tests for `CacheScope`, `$schema` normalization, structured shape** — `53e51d1a` (test)
3. **Task 3: the runnable example** — `dcdd8a2a` (docs)
4. **Task 4: full quality gate** — no code; evidence transcribed below, deferred items appended to `deferred-items.md`

**Plan metadata:** see the final `docs(115-09)` commit.

## Files Created/Modified

**Created**
- `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` (337 lines) — the target, its byte layout, `is_dialect_neutral`, and the in-code explanation of why monotonicity is not asserted
- `fuzz/corpus/fuzz_schema_draft_pin/{01..11}_*` — 11 hand-written seeds
- `fuzz/corpus/fuzz_schema_draft_pin/README.md` — the byte layout, a `python3` recipe for adding a case, and a per-file table
- `examples/s52_v2_caching_hints.rs` (471 lines) — the ALWAYS example

**Modified**
- `src/server/output_validation.rs` — `pub mod fuzz_support` (`SchemaVerdict`, `validate_bytes`, `normalize_bytes`) + 5 rot-proof in-module tests
- `src/server/mod.rs` — the twin-cfg widening of `output_validation`
- `src/types/caching.rs` — `#[cfg(test)] ALL_SCOPES` beside the enum + `mod caching_properties` with two proptest blocks
- `tests/property_tests.rs` — `arb_json` made visible to siblings, one new property in `structured_output_invariants`, one new `fuzzing`-gated module
- `fuzz/Cargo.toml` — `validation` added to the `pmcp` feature list; `[[bin]]` registration
- `fuzz/.gitignore` — narrowed so this target's seeds can be tracked
- `Cargo.toml` — the `[[example]]` block

## Verification Evidence

Every number below came from a command that can fail. The three fail-open `make`
targets are cited only as findings, never as evidence.

### `make quality-gate` — EXIT 0

Run twice. The first attempt failed on disk exhaustion (see Issues); the second,
after freeing space, printed `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED`. Every
constituent step was ALSO run individually, which is what produced the per-step
transcript below (`quality-gate` is literally twelve `$(MAKE) <step>` lines):

| Step | Exit | Evidence |
|------|------|----------|
| `fmt-check` | 0 | `✓ Code formatting OK` |
| `lint` | 0 | `✓ No lint issues` (lib clippy + `--examples` under `-D warnings`) |
| `build` | 0 | `--all-features`, 13.56 s |
| `test-unit` | 0 | `1807 passed; 0 failed` |
| `test-doc` | 0 | `417 passed; 0 failed; 79 ignored` (354.54 s) |
| `test-property` | 0 | **vacuous — see D-115-09-B**: 101 × `ok. 0 passed`, zero lines with a non-zero pass count |
| `test-examples` | 0 | 81 examples built (never run — D-115-09-C) |
| `test-integration` | 0 | `✓ Integration tests passed`, no `test result: FAILED` |
| `pmcp-package-gate` | 0 | fmt + clippy + test on the workspace-excluded crate |
| `audit` | 0 | `✓ No vulnerabilities found` |
| `unused-deps` | 0 | skipped (`cargo machete` not installed) — pre-existing |
| `check-todos` | 0 | `✓ No technical debt comments` |
| `check-unwraps` | 0 | `✓ No unwrap() calls in production code` |
| `validate-always` | 0 | re-runs fuzz/property/unit/examples — three of the four are fail-open |
| `purity-check` | 0 | reader-free + writer-present + cargo-deny bans clean |
| `comply` | 0 | all four team-servers bindings resolve |

Additionally `make wasm-build` → exit 0, and
`git diff --stat -- Makefile .github/ deny.toml` → **empty**. No gate was
weakened; the only ignore-file edit is `fuzz/.gitignore` (D-115-09-D).

### The fuzz target

```
$ cd fuzz && cargo +nightly fuzz run fuzz_schema_draft_pin -- -runs=0 corpus/fuzz_schema_draft_pin
INFO:       12 files found in corpus/fuzz_schema_draft_pin
#28  DONE   cov: 8394 ft: 13703 corp: 22/8260b lim: 4468 exec/s: 0 rss: 92Mb
exit 0        artifacts/fuzz_schema_draft_pin/: 0 files
```

```
$ cd fuzz && cargo +nightly fuzz run fuzz_schema_draft_pin -- -max_total_time=60 <scratch> corpus/fuzz_schema_draft_pin
#629244  DONE  cov: 10586 ft: 25778 corp: 1549/266Kb lim: 4468 exec/s: 10315 rss: 634Mb
Done 629244 runs in 61 second(s)
exit 0        artifacts/fuzz_schema_draft_pin/: 0 files
```

**Observed: 629,244 execs, ~10,315 exec/s, coverage 10,586, features 25,778, no
crash artifact.** The corpus was written to a scratch directory so the committed
seeds stayed byte-identical (`git status --short fuzz/corpus/` clean afterwards).
Coverage rising from 8,394 (seeds only) to 10,586 (after mutation) is the
measurement that the seeds put the fuzzer INSIDE schema compilation rather than
in `serde_json`'s parser.

### Negative control — the vacuous-pin regression (transcribed)

`compile_for_era`'s `Era::V2` arm was temporarily changed to
`jsonschema::draft202012::new(schema)` — i.e. the pin WITHOUT the normalization
step — and the corpus replayed:

```
thread '<unnamed>' panicked at fuzz_targets/fuzz_schema_draft_pin.rs:294:5:
assertion `left == right` failed: DIALECT-NEUTRAL ERA DISAGREEMENT. This schema uses only
keywords whose meaning is identical in draft-07 and 2020-12, so both eras must reach the same
verdict. The usual cause is the vacuous-validator bypass: a legacy `$schema` declaration
compiled under the 2020-12 pin WITHOUT the normalization step yields an empty vocabulary set,
producing a validator that accepts everything — v2 says `Conforms` where v1 says `Violates`.
Restore the normalize-then-pin step in `compile_2020_12`.
schema: {"$schema":"http://json-schema.org/draft-07/schema#","type":"object",
         "properties":{"n":{"type":"integer"}},"required":["n"]},
instance: <same document, self-pair>
  left: Violates
 right: Conforms
...
SUMMARY: libFuzzer: deadly signal
Test unit written to fuzz/artifacts/fuzz_schema_draft_pin/crash-3eec6c50ce166e86e590e35e2a52aaa185111c08
Error: Fuzz target exited with exit status: 77
```

Reverted immediately; `Era::V2 => compile_2020_12(schema)` restored, the crash
artifact deleted, and the replay re-run green (exit 0, 0 artifacts). The failing
seed was `02_draft07_object_conforming` under the JSON-family self-pair, which is
the same defect `01_draft07_object_violating` catches with an explicit instance —
libFuzzer simply reached it first.

### Test counts

| Command | Before | After |
|---------|--------|-------|
| `nextest --features full -E 'binary(property_tests)'` | 16 | **17** (+1: test 4 only; test 3 is `fuzzing`-gated) |
| `nextest --features "full fuzzing" -E 'binary(property_tests)'` | 16 | **18** (+2: tests 3 and 4) |
| `nextest --lib --features full -E 'test(/types::caching/)'` | 13 | **15** (6 projection + 7 serde locks + 2 new properties) |
| `nextest --lib --features "full fuzzing" -E 'test(/output_validation::fuzz_support/)'` | — | **5**, all passing |
| `nextest --lib --features "full fuzzing" -E 'test(/fuzz_support/)'` | 1 | **6** — see D-115-09-G |

The two `binary(property_tests)` counts differ by exactly 1, as the plan
required.

### `cargo public-api diff` — the seam does not ship

```
$ cargo public-api --package pmcp diff --deny=all 0936f46e..HEAD
Removed items from the public API   (none)
Changed items in the public API     (none)
Added items to the public API       (none)
```

Zero public-API change across the whole plan. The broader
`cargo public-api --package pmcp diff` (against published 2.17.0) shows 3,065
additions, all of them pre-existing Phase 113/114/115 work; grepping it for
`fuzz_support`, `SchemaVerdict`, `output_validation`, `ALL_SCOPES`,
`validate_bytes` and `normalize_bytes` returns nothing. T-115-25 mitigated and
measured.

### The example

`timeout 30s cargo run --example s52_v2_caching_hints --features full` → **exit 0**,
no input awaited. Stdout (abridged only where a `_meta` blob repeats):

```
server listening on http://127.0.0.1:51466/

1. resources/list — the HANDLER set the posture (v2)
  resources/list
    ttlMs       = 300000
    cacheScope  = "public"
    raw         = {"resources":[{"uri":"docs://catalogue",...},{"uri":"docs://me/profile",...}],
                   "ttlMs":300000,"cacheScope":"public","resultType":"complete","_meta":{...}}
    -> `public` authorizes a shared gateway to serve this body
       across authorization contexts. Correct here (the catalogue is identical
       for every caller); a data leak on a per-user body.

2. resources/read and tools/list — the SDK DEFAULT (v2)
  resources/read
    ttlMs       = 0
    cacheScope  = "private"
  tools/list
    ttlMs       = 0
    cacheScope  = "private"
    -> `ttlMs: 0` means "immediately stale", so the default asserts NOTHING
       about cacheability — inert, yet the v2 wire's required keys are present.

3. the SAME server answering a 2025-11-25 client — NEITHER key
  (v1 handshake completed; session f18d3f18-… — v2 needed neither)
  resources/list (v1)
    ttlMs       = <absent>
    cacheScope  = <absent>
    raw         = {"resources":[{"uri":"docs://catalogue",...},{"uri":"docs://me/profile",...}]}
  tools/list (v1)
    ttlMs       = <absent>
    cacheScope  = <absent>
    -> the handler SET a hint on resources/list, and the v1 projection STRIPPED it.
       Not "did not add" — actively removed, so the legacy wire is unchanged.

4. tools/call — NON-OBJECT structuredContent (v2)
    raw         = {"content":[{"type":"text","text":"42"}],"isError":false,
                   "structuredContent":42,"resultType":"complete","_meta":{...}}
    -> `structuredContent: 42`. A scalar, not `{"value": 42}`.
  CallToolResult::structured_value(json!(42))   => {...,"structuredContent":42}
  CallToolResult::structured_value(json!(null)) => {...,"structuredContent":null}

all four demonstrations asserted — exiting 0
```

Note the v1 `raw` lines: no `ttlMs`, no `cacheScope`, and no `resultType` /
`_meta` envelope either — a byte-identical legacy response from a server that is
simultaneously answering v2.

**Example negative control:** the v1 `ttlMs` assertion was inverted to
`is_some()`; the run exited **101** with
`resources/list: a v1 response must never carry ttlMs, got {"resources":[…]}`.
Reverted; the run is green again. `s52` was free — `s51_v2_tasks_agent` remains
the highest prior prefix — so no renumbering was needed.

## Decisions Made

- **D-115-09-1: invariant 3 is an equality over a neutrality predicate, and the predicate is TIGHTER than the plan specified.** The plan's literal rule ("every object key at every level must be in the allowlist or be `$schema`") would have made `properties`' author-chosen child names non-neutral, excluding the very seed the negative control has to fire on. The implemented predicate recurses into `properties`' VALUES (not its keys) and into `additionalProperties`, and adds two restrictions the plan did not state: the root `$schema` must be absent, draft-07 or 2020-12, and a NESTED `$schema` is disqualifying. Both additions are load-bearing — under draft-04 the allowlisted keyword `type: "integer"` rejects `1.0`, which draft-06 onwards accepts, so a draft-04 declaration is a genuine counterexample to the equality; and a nested `$schema` is a per-resource dialect switch from 2019-09 but merely ignored in draft-07. Without them the fuzzer would eventually report a TRUE divergence as a bug.
- **D-115-09-2: the selector byte is behavioural, not decorative.** On the JSON family the target also validates the schema against ITSELF (a JSON Schema document is a JSON instance), doubling semantic coverage from one input; on the raw family it does not, because that is pointless on garbage. A byte the target ignored would be dead input.
- **D-115-09-3: the `Some(Value::Null)` round trip is asserted as measured, not fixed.** Documented under Deviations.
- **D-115-09-4: `ALL_SCOPES` is `#[cfg(test)]`.** The plan asked for it "beside the enum"; making it `pub` would have put a new item in a phase whose `public-api diff` is expected to be empty, and the repo's own precedent (`types::tasks::ALL_STATUSES`) is a test-scoped enumeration. It is declared immediately below the enum, which is what "beside" was buying.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The plan's `dependencies` divergence example is false; corrected to `contentEncoding`**
- **Found during:** Task 1 (the seam's in-module tests)
- **Issue:** Task 1(a) mandated `fuzz_support_reports_the_divergent_dependencies_case_asymmetrically`, asserting `Some((Violates, Conforms))` for a draft-07-declared `{"dependencies": {"a": ["b"]}}`. 115-03 measured on `jsonschema` 0.49.2 that the crate still honours `dependencies` under the 2020-12 pin, so both eras agree and the test would have failed — exactly the inheritance D-115-03-C was raised to prevent.
- **Fix:** Renamed to `..._the_divergent_content_encoding_case_asymmetrically` and asserted the MEASURED case: `contentEncoding` is an assertion in draft-07 and an annotation from 2019-09, so a non-base64 string gives `(Violates, Conforms)`. The test passes. `dependencies` remains in the target's EXCLUDED-keyword list (the spec-level split is real) and keeps seed `05`, which now exercises the neutrality skip; the README states the distinction.
- **Files modified:** `src/server/output_validation.rs`, `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`, `fuzz/corpus/fuzz_schema_draft_pin/README.md`
- **Verification:** the seam test passes; `-E 'test(/output_validation::fuzz_support/)'` → 5/5
- **Committed in:** `ddfb450b`

**2. [Rule 3 - Blocking] `cargo fuzz` requires `+nightly`; the plan (and the Makefile) use the plain form**
- **Found during:** Task 1(e), the first corpus replay
- **Issue:** The plan instructed "no `+nightly` — it must match the Makefile `test-fuzz` target". `cargo fuzz` passes `-Zsanitizer=address`, which stable rustc refuses outright: `error: the option 'Z' is only accepted on the nightly compiler`. The build fails before any iteration. This is repo-wide, not specific to this target.
- **Fix:** Used `cargo +nightly fuzz run` for every invocation and corrected the target's module doc, which now states that `+nightly` is required, why, and that `make test-fuzz` therefore reports success having fuzzed nothing.
- **Files modified:** `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`
- **Verification:** replay and 60 s run both exit 0 under `+nightly`; `make test-fuzz` measured to exit 0 with 20 build failures swallowed (booked as D-115-09-A)
- **Committed in:** `ddfb450b`

**3. [Rule 3 - Blocking] `fuzz/corpus` was gitignored, so the required committed corpus could not exist**
- **Found during:** Task 1, staging
- **Issue:** `fuzz/.gitignore:2` ignored `corpus` wholesale. `git add` refused all 11 seeds. The plan's `must_haves`, its acceptance criteria and threat T-115-41 all require a COMMITTED corpus.
- **Fix:** Narrowed the ignore: `corpus/*` still ignores everything, `!corpus/fuzz_schema_draft_pin/` re-includes this target's directory, `corpus/fuzz_schema_draft_pin/*` re-excludes its contents, and two negations track only `README.md` and `[0-9][0-9]_*`. libFuzzer's SHA-named runtime units stay ignored, so a local fuzzing session never dirties the tree — verified by `git status --short fuzz/corpus/` being clean after a 60 s run that produced 1,549 corpus entries.
- **Files modified:** `fuzz/.gitignore`
- **Verification:** 11 seeds + README tracked; working tree clean after fuzzing
- **Committed in:** `ddfb450b` (booked as D-115-09-D)

**4. [Rule 3 - Blocking] `fuzz/Cargo.lock` could not resolve `validation`'s `jsonschema`**
- **Found during:** Task 1(b)
- **Issue:** Adding the existing in-repo `validation` feature pulled `jsonschema`, which needs `getrandom ^0.3.4`, against a lock pinning `getrandom 0.3.3` for `uuid`. A targeted `cargo update -p getrandom@0.3.3` then hit a second stale pin via `regex-automata` / `mcp-tester`.
- **Fix:** `cargo update` inside `fuzz/`. The lockfile is gitignored, so nothing was committed and no manifest constraint changed. `jsonschema` resolves to **0.49.2**, matching the root lock and the version every measurement in this phase was taken on.
- **Files modified:** none tracked (`fuzz/Cargo.lock` is gitignored)
- **Verification:** the fuzz crate builds; `grep jsonschema fuzz/Cargo.lock` → 0.49.2
- **Committed in:** n/a (booked as D-115-09-E)

**5. [Rule 3 - Blocking] Disk exhaustion (227 MiB free) fabricated 9 test failures**
- **Found during:** Task 4
- **Issue:** The first `make quality-gate` failed with 8 `session_validation_tests` failures panicking at the pre-existing native-root-certificates `.expect` with a keychain `Os { code: -36 }` I/O error, and a later `test-unit` run failed the `sse_parser` linear-growth perf test at 10.29x (ceiling 8.0x) with absolute timings 6–20× the documented shape. Neither is a code defect. `target/debug/incremental` was 35 GiB.
- **Fix:** Removed `target/debug/incremental`, `target/semver-checks`, `target/doc` and (later) `fuzz/target`, freeing ~37 GiB, and re-ran with `CARGO_INCREMENTAL=0`. Both suites pass in isolation on a healthy volume (`session_validation_tests`: 10/10; the perf test: 1/1) and the full gate then passes.
- **Files modified:** none
- **Verification:** `make quality-gate` exit 0 on the second run
- **Committed in:** n/a (booked as D-115-09-F)

**6. [Rule 2 - Missing Critical] `required-features` extended with `testing`**
- **Found during:** Task 3
- **Issue:** The plan specified `required-features = ["streamable-http", "http-client"]`. The reserved `_meta` key spellings (`io.modelcontextprotocol/protocolVersion` and siblings) are `pub(crate)` in `types::protocol::context` and reachable publicly ONLY through `pmcp::testing`, which the `testing` feature gates. Without it the example would have to hand-copy three wire strings — precisely the drift `tests/common/v2.rs:673-681` was already bitten by.
- **Fix:** `required-features = ["streamable-http", "http-client", "testing"]`, with the reason stated in the `[[example]]` block's comment. `testing` is folded into `full`, so the documented run command is unaffected, and `s47_v2_stateless_mrtr` already uses `testing` in its own list.
- **Files modified:** `Cargo.toml`
- **Verification:** `cargo build --examples --features full` exit 0; `make test-examples` builds it
- **Committed in:** `dcdd8a2a`

### Deliberate departures from the plan text (not auto-fixes)

**7. The seam test count is 6 under the plan's selector, 5 under a precise one.**
`-E 'test(/fuzz_support/)'` also matches the pre-existing
`server::request_state::tests::fuzz_support_seam_rejects_garbage`, so "exactly 5"
is unachievable. The new module is named `fuzz_support_tests` and
`-E 'test(/output_validation::fuzz_support/)'` selects exactly the 5 new ones.
Both counts are recorded above (D-115-09-G).

**8. `grep -c 'cached_validator'` inside `fuzz_support` returns 2, not 0.**
Both are rustdoc prose — the plan itself requires the rustdoc to explain why the
seam does not use the cache. `grep -c 'cached_validator('` (a CALL) returns 0,
which is what the criterion is about.

**9. `structuredContent` round-trip: `Some(Value::Null)` collapses to `None`.**
The plan asserted the round trip preserves every shape "INCLUDING the
`Value::Null` case". The EMIT half holds (the wire carries an explicit
`"structuredContent":null`, asserted); the PARSE half does not, because
`Option<Value>`'s stock `Deserialize` maps JSON `null` to `None`. This is
pre-existing, already measured and fenced by 115-04 (**D-115-04-A**), and the fix
is a `deserialize_with` double-`Option` on a shipped public type in
`src/types/tools.rs` — a file outside this plan's `files_modified`, changing how
every client parses every tool result on both eras. The property therefore holds
the round trip over the non-null shapes and asserts the measured behaviour for
null, with a message telling a future fixer to delete that branch.

**10. The example demonstrates `structured_value` in-process, not from the tool handler.**
A `ToolHandler` returns a plain `Value` that the dispatcher bridges into
`structuredContent`; returning a serialized `CallToolResult` from one would trip
the TOUT-02 double-wrap tripwire (a `debug_assert` in debug builds). The example
therefore shows the wire via the declared-`outputSchema` tool AND the constructor
directly, with a comment naming which layer builds what.

---

**Total deviations:** 6 auto-fixed (1 bug, 4 blocking, 1 missing-critical) + 4
documented departures.
**Impact on plan:** No scope creep. Four of the six auto-fixes are environmental
or tooling blockers; the one code-level bug fix (deviation 1) is the correction
115-03 explicitly asked this plan to make. No production behaviour changed —
`cargo public-api diff` across the plan is empty and the only `src/` edits are a
test-only seam and test modules.

## Issues Encountered

- **The harness cannot hold `make quality-gate` in one foreground call.** The chain takes ~20 minutes against a 600 s limit; a `setsid nohup` detachment was killed silently and wrote no log. Resolved by running the twelve constituent targets individually (which IS the chain — `quality-gate` has no other logic) and then the whole chain once via the background runner with the exit code written to a file, which completed with exit 0.
- **Busy-wait polling starved a timing-sensitive test.** An `until ! pgrep …; do :; done` loop consumed a core while the gate ran and is the likely trigger for the `sse_parser` perf failure. Replaced with `sleep`-based waits. Worth remembering alongside the existing "do not run two cargo invocations concurrently" note.
- **A near-miss deletion.** `tests/property_tests.proptest-regressions` — a TRACKED file with two historical regression seeds — was removed by an `rm -f` while clearing a proptest failure artifact. Caught by the post-commit deletion check before it was staged and restored with `git checkout --`. `git diff --diff-filter=D HEAD~2 HEAD` is empty.

## Deferred Items Booked

Eight new entries appended to
`.planning/phases/115-.../deferred-items.md` under **From 115-09**:
D-115-09-A (`make test-fuzz` is fail-open twice over — it never runs a fuzzer at
all on a stable toolchain), D-115-09-B (`make test-property` selects zero tests,
now measured: 101 × `0 passed`), D-115-09-C (`make test-examples` never executes),
D-115-09-D (`fuzz/corpus` gitignore), D-115-09-E (stale `fuzz/Cargo.lock`),
D-115-09-F (disk exhaustion → 9 phantom failures), D-115-09-G (the 5-vs-6 seam
selector), D-115-09-H (the inherited `dependencies` example).

## User Setup Required

None — no external service configuration required. Contributors running the fuzz
target locally need a nightly toolchain (`rustup toolchain install nightly`),
which is already present in this environment and is stated in the target's module
doc.

## Next Phase Readiness

- CLAUDE.md's ALWAYS quartet is now satisfied for all three of this phase's
  features: FUZZ (`fuzz_schema_draft_pin`), PROPERTY (4 new tests across two
  files), UNIT (115-03/05/06/07) and EXAMPLE (`s52_v2_caching_hints`).
- 115-10 has a substantially larger deferred ledger than it expected. Two entries
  are repo-wide gate defects with real consequences (A and B) and one is a
  research-document correction that 115-03 already requested (H).
- Nothing here blocks 115-10. The phase's production surface is unchanged by this
  plan, which `cargo public-api diff 0936f46e..HEAD` proves.

---
*Phase: 115-json-schema-2020-12-structured-output-caching-hints*
*Completed: 2026-08-01*

## Self-Check: PASSED

All claimed artifacts exist on disk and all three task commits resolve:

- FOUND `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`
- FOUND `fuzz/corpus/fuzz_schema_draft_pin/README.md` (+ 11 numbered seeds)
- FOUND `examples/s52_v2_caching_hints.rs`
- FOUND `.planning/phases/115-.../115-09-SUMMARY.md`
- FOUND commits `ddfb450b`, `53e51d1a`, `dcdd8a2a`
