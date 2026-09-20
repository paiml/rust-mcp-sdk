---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 04
subsystem: api
tags: [structured-content, call-tool-result, era-v2, test-harness, anti-vacuity, schm-02]

# Dependency graph
requires:
  - phase: 115-01
    provides: the vendored 2026-07-28 core schema whose `structuredContent?: unknown` declaration and doc comment this plan's rustdoc quotes verbatim
  - phase: 115-02
    provides: raw-byte v1 list/read goldens — nothing here changes a wire byte, and they stayed green
  - phase: 115-11
    provides: the `structured_content_shape` contract equation and its two bindings (`structured_value` planned, `structured` frozen), written contract-first
  - phase: 112
    provides: "`ProtocolContext` / `Era`, the accept-list opt-in, and `inject_v2_result_envelope` — the `resultType` key that makes the era MEASURABLE from a test"
provides:
  - "`CallToolResult::structured_value` — the D-06 sibling constructor for non-object payloads, with a doctest"
  - the era rule in rustdoc on `structured_content`, `structured` and `structured_value`, including the frozen-v1-permissiveness consequence
  - "an era-aware in-process test seam in `tests/common/duplex.rs` (`v2_accept_list`, `call_tool_request`, `raw_via_core`, `raw_via_server`, `initialize_via_core`, `result_object`, `assert_v2_witness`, `assert_no_v2_witness`, `call_tool_result_of`)"
  - non-object structuredContent coverage on BOTH dispatchers on BOTH eras, with the era proven in-band
  - "an anti-vacuity test proving the v2 witness discriminates opted-in from non-opted-in"
  - "the measured finding that a present `structuredContent: null` does not survive a typed re-read"
affects: [115-06, 115-07, 115-09, 115-10]

# Tech tracking
tech-stack:
  added: []   # no package installed, no manifest touched
  patterns:
    - "Era-aware in-process dispatch: opt the fixture in AND signal the era AND measure the resolved era; two of the three is a vacuous test"
    - "An in-band server-minted key (`resultType`) is a stronger era witness than any test-side bookkeeping"
    - "Assert present-vs-absent on the RAW result map, where `Map::get` can express it — a typed `Option<Value>` re-read cannot"

key-files:
  created: []
  modified:
    - src/types/tools.rs
    - tests/common/duplex.rs
    - tests/structured_tool_output.rs
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md

key-decisions:
  - "Finding 6 CONFIRMED on the tree as it stands: there is no object-only guard on the structured-content path to remove. The only non-test `is_object()` in either dispatcher is inside `inject_v2_result_envelope` (a JSON-RPC result-envelope guard), and `src/types/tools.rs` / `src/server/output_validation.rs` have zero"
  - "`CallToolResult::structured` is byte-unchanged; the widening is the sibling `structured_value`, whose body is identical and whose rustdoc is the entire difference (D-06)"
  - "The null dispatcher tests assert on the RAW result object rather than a re-read `CallToolResult`, because serde collapses a present JSON null onto `Option::None` — the plan's `Some(null)` vs `None` intent is expressible at the map level and nowhere else"
  - "The serde collapse was NOT fixed: it is pre-existing, is not a wire defect, and changing it would alter the client-side meaning of every `CallToolResult` on BOTH eras. Recorded as a tripwire test and deferred to 115-10"
  - "`initialize_via_core` was added (not in the plan): `ServerCore` gates a v1 request behind the initialize handshake while a v2 request needs none, so the v1 half of every core pair needs the handshake"
  - "`contracts/binding.yaml` was NOT edited — same posture 115-03 took; the `structured_value` binding stays `planned` and is booked as D-115-04-B"

patterns-established:
  - "Pattern: a test that claims an era must prove it from a server-minted signal in the response, never from the request it sent"
  - "Pattern: pair every era claim with its negative twin (`assert_no_v2_witness`), so the two halves differ by configuration as well as by name"
  - "Pattern: revert a temporary negative control from a file COPY, never `git checkout --` on a file with uncommitted work"

requirements-completed: [SCHM-02]

# Metrics
duration: 75min
completed: 2026-08-01
---

# Phase 115 Plan 04: Non-Object `structuredContent` on v2 Summary

**SCHM-02 shipped as what it measurably is — a sibling constructor, an era statement in rustdoc, and dispatcher tests whose v2 claim is proven by the server-minted `resultType` key rather than assumed — with no guard removed, because 115-RESEARCH Finding 6 was confirmed: there is none.**

## Performance

- **Duration:** ~75 min
- **Started:** 2026-08-01T00:15:00Z (approx; first commit 00:40 local)
- **Completed:** 2026-08-01T01:30:00Z
- **Tasks:** 3
- **Files modified:** 4 (1 source + 2 test + 1 planning ledger)

## Accomplishments

- **`CallToolResult::structured_value` exists, is documented with the v2 quote verbatim, and carries a passing doctest.** `cargo semver-checks check-release` reports *"no semver update required"* and `cargo public-api diff` names it as the ONLY added item, with zero removed and zero changed — the T-115-13 additive-only obligation, measured.
- **`CallToolResult::structured` is byte-unchanged.** `git diff src/types/tools.rs | grep '^-' | grep -c 'pub fn structured(value: Value)'` → `0`. Fenced going forward by `structured_keeps_its_object_shaped_intent`.
- **The pre-review vacuity defect is closed, and closed with a measurement.** Every v2 test opts its fixture in via `.with_supported_protocol_versions(v2_accept_list())`, signals the era through pmcp's own `RequestMeta`, and calls `assert_v2_witness` FIRST. Negative control A proves the opt-in is load-bearing; `structured_output_the_v2_witness_is_load_bearing` proves the witness discriminates.
- **Scalar, array and null `structuredContent` round-trip through BOTH dispatchers on `Era::V2`**, and the v1 path is proven unchanged BY CONTRAST on both — which is what makes the v2 half mean anything (D-05's freeze asserted, not assumed).
- **D-04's warn-only posture proven at the dispatcher level**: an object-shaped `outputSchema` against a scalar payload still returns a success `Result` payload with the value verbatim.
- **A real defect surfaced that the plan did not anticipate** (see § Findings 2): a present `structuredContent: null` reaches the wire correctly but collapses to `None` on a typed re-read. Recorded as a tripwire test and deferred, not silently absorbed and not unilaterally "fixed".
- **Full `make quality-gate` exit 0** — 4954 passed / 0 failed / 81 ignored across 303 `test result: ok` lines. This checkout has no `.git/hooks/pre-commit`, so nothing would have run it otherwise.

## Task Commits

1. **Task 1: `structured_value` sibling constructor + era rustdoc** — `9c7746c3` (feat), `src/types/tools.rs`
2. **Task 2: era-aware dispatch helpers + non-object coverage on both eras** — `890e3410` (test), `tests/common/duplex.rs`, `tests/structured_tool_output.rs`
3. **Task 3: full quality gate** — no code change; results transcribed below and committed with this SUMMARY

## Files Created/Modified

- `src/types/tools.rs` — `structured_value` placed directly after `structured_with_text` (body identical to `structured`); `# Era` rustdoc on the `structured_content` FIELD with the v2 quote, the v1 spec text it replaces, and the frozen-v1-permissiveness paragraph; a cross-reference line added to `structured`'s rustdoc. +108/-1.
- `tests/common/duplex.rs` — the era-aware seam (below the two existing helpers, which are byte-unchanged), plus a module-doc section explaining why the second seam exists at all. +267.
- `tests/structured_tool_output.rs` — 14 new tests (6 → 20), a fourth claim in the module doc, and the `era_aware` module. +480.
- `.planning/phases/.../deferred-items.md` — three new entries (D-115-04-A…C).

## Recorded Measurements

### Finding 6 confirmed — there was no guard to remove

Re-measured on the tree as it stands, not taken from the research:

```
$ grep -c "is_object()" src/types/tools.rs src/server/output_validation.rs
0    0
$ grep -n "is_object()" src/server/core.rs   # non-test hits
1593:    if !value.is_object() {     # inside inject_v2_result_envelope — a JSON-RPC
                                     # RESULT-envelope guard ("cannot key a non-object"),
                                     # nothing to do with structuredContent
$ grep -n "is_object()" src/server/mod.rs    # non-test hits: none (3 hits, all in #[cfg(test)])
```

The executor did NOT go looking for a bridge to dismantle, and none was added.

### The `structured_value` doctest (Task 1)

```
$ cargo test --doc --features full structured_value -- --list
src/types/tools.rs - types::tools::CallToolResult::structured_value (line 754): test

$ cargo test --doc --features full structured_value
cargo test: 1 passed, 488 filtered out (1 suite, 0.85s)

$ cargo test --doc --features full
cargo test: 410 passed, 79 ignored (1 suite, 136.48s)
```

### The additive-only check (T-115-13)

```
$ cargo semver-checks check-release --package pmcp --baseline-rev HEAD
    Checking pmcp v2.17.0 -> v2.17.0 (no change; assume patch)
     Checked [   0.181s] 223 checks: 223 pass, 30 skip
     Summary no semver update required

$ cargo public-api --features full diff HEAD~1..HEAD
Removed items from the public API
=================================
(none)

Changed items in the public API
===============================
(none)

Added items to the public API
=============================
+pub fn pmcp::types::CallToolResult::structured_value(serde_json::value::Value) -> Self
   (listed once per re-export path; ONE item)
```

`cargo public-api diff` cannot diff a dirty working tree against `HEAD` (it requires
`ref1..ref2`), so this was run after the Task 1 commit rather than before it.

### The suite (Task 2 / Task 3)

```
$ cargo nextest run --features full -E 'binary(structured_tool_output)'
     Summary [   0.031s] 20 tests run: 20 passed, 0 skipped
```

20 = 6 pre-existing + 14 new (10 dispatcher tests in 5 `server_` / `server_core_` pairs,
1 anti-vacuity, 3 unit-level). The plan asked for ≥19.

Acceptance greps, all measured:

| Check | Required | Measured |
|---|---|---|
| `grep -c 'server_core_' tests/structured_tool_output.rs` | ≥7 | 7 |
| `grep -c 'assert_v2_witness' tests/structured_tool_output.rs` | ≥5 | 11 |
| `grep -c 'assert_no_v2_witness' tests/structured_tool_output.rs` | ≥3 | 4 |
| `grep -c 'io.modelcontextprotocol/protocolVersion' tests/common/duplex.rs` | 0 | 0 |
| `grep -c 'pub fn structured_value' src/types/tools.rs` | 1 | 1 |
| `grep -c 'any JSON value (object, array, string, number, boolean, or null)' src/types/tools.rs` | ≥1 | 2 |
| `git diff --stat -- Makefile .github/ deny.toml` | empty | empty |

### Negative control A — remove the v2 opt-in from `v2_core`

Deleted `.with_supported_protocol_versions(v2_accept_list())` from the `v2_core` fixture.
Observed **5 failures** — every `server_core_*` v2 test plus the anti-vacuity test:

```
     Summary [   0.030s] 15/20 tests run: 10 passed, 5 failed, 0 skipped
        FAIL era_aware::server_core_v2_array_structured_content_survives_round_trip
        FAIL era_aware::server_core_v2_null_structured_content_is_present_not_omitted
        FAIL era_aware::server_core_v2_object_schema_with_scalar_payload_still_returns_a_result
        FAIL era_aware::server_core_v2_scalar_structured_content_survives_round_trip
        FAIL era_aware::structured_output_the_v2_witness_is_load_bearing

thread 'era_aware::structured_output_the_v2_witness_is_load_bearing' panicked at tests/common/duplex.rs:348:13:
expected a Result payload, got error: JSONRPCError { code: -32002,
  message: "Server not initialized. Call initialize first.", data: None }
```

**The failure is even sharper than the plan predicted.** Without the opt-in the request is not
merely served as v1 — it is refused by the v1 initialize gate, because
`v1_initialize_gate_applies` returns `true` once the era is no longer `Some(Era::V2)`. So the
opt-in is load-bearing twice over: for the envelope AND for the lifecycle. Reverted.

### Negative control B — add an object-only guard to `ServerCore`

Inserted, immediately inside the declared-`outputSchema` branch at `src/server/core.rs:834`:

```rust
if !value.is_object() {
    return Ok(ToolCallOutcome::Result(CallToolResult::new(vec![])));
}
```

Observed **6 failures**, all `ServerCore`-side, on both eras:

```
     Summary [   0.020s] 15/20 tests run: 9 passed, 6 failed, 0 skipped
        FAIL era_aware::server_core_v1_scalar_structured_content_is_unchanged
        FAIL era_aware::server_core_v2_array_structured_content_survives_round_trip
        FAIL era_aware::server_core_v2_null_structured_content_is_present_not_omitted
        FAIL era_aware::server_core_v2_object_schema_with_scalar_payload_still_returns_a_result
        FAIL era_aware::server_core_v2_scalar_structured_content_survives_round_trip
        FAIL era_aware::structured_output_the_v2_witness_is_load_bearing

panicked at tests/structured_tool_output.rs:477:9:
  left: None
 right: Some(Number(42))
panicked at tests/structured_tool_output.rs:513:9:
  left: None
 right: Some(Array [String("a"), String("b")])
panicked at tests/structured_tool_output.rs:563:9:
  left: None
 right: Some(Null)
```

Note the v1 twin failed too — which is the point of D-05: a guard added "for v2 correctness"
would have broken v1 as well. The `Server`-side tests stayed green, confirming the twin-site
split is real and that a `ServerCore`-only change leaves half the SDK unproven (Pitfall 6).
Reverted from a file copy (see § Deviations 3).

### `make quality-gate` (Task 3)

```
$ /usr/bin/make quality-gate            # exit 0
fmt-check          ✓
lint               ✓ No lint issues        (clippy --lib --tests, RUSTFLAGS="-D warnings")
build              ✓
test-all           ✓ 4954 passed / 0 failed / 81 ignored, 303 `test result: ok` lines,
                     0 `test result: FAILED` lines
                     └─ structured_tool_output: 20 passed; 0 failed; 0 filtered out
pmcp-package-gate  ✓
audit              ✓
unused-deps        ✓ (cargo machete not installed — skipped, as always)
check-todos        ✓ No technical debt comments
check-unwraps      ✓
validate-always    ✓ all examples built
purity-check       ✓
comply             ✓ CB-1338: 45 binding(s) verified, 0 ghosts
                     ✓ every team-servers binding resolves to a real function
        ✅ ALL TOYOTA WAY QUALITY CHECKS PASSED
```

**First attempt was a false red, caused by the executor, not by the change.** Two
`make quality-gate` runs overlapped (the first had been backgrounded on timeout), and
`tests/test_websocket_server.rs` binds a hardcoded `127.0.0.1:9005`:

```
Failed to bind: Internal("Failed to bind to 127.0.0.1:9005: Address already in use (os error 48)")
make[1]: *** [test-integration] Error 101
```

`cargo test --features full --test test_websocket_server` in isolation → **6 passed**, and the
clean re-run above is green. Recorded because "the gate went red once" belongs on the record
even when the cause was operator error.

## Findings

### 1. `ServerCore` gates v1 behind `initialize`; v2 needs no handshake — and the plan's helper design had to account for it

`v1_initialize_gate_applies` (`src/server/core.rs:3936`) returns `true` for a v1 /
non-opted-in request on a non-stateless core, so a v1 `tools/call` sent as the FIRST message
gets `-32002 "Server not initialized. Call initialize first."` rather than a result. The plan
anticipated this question only for `raw_via_server`; the answer differs per dispatcher:

| Dispatcher | v2 request as first message | v1 request as first message |
|---|---|---|
| `ServerCore` | works — no handshake (HTTP-01) | `-32002`, needs `initialize_via_core` |
| high-level `Server` | works | **works** — this dispatcher has no initialize gate at all |

So `raw_via_server` needed no handshake for EITHER era (the plan's contingency did not fire),
and `initialize_via_core` was added for the v1 half of every `ServerCore` pair. The asymmetry
is itself corroborating evidence that the era reached the dispatcher.

### 2. A present `structuredContent: null` does not survive a typed re-read (deferred)

The plan expected `structured_content == Some(Value::Null)` after a null round-trip. Measured,
it comes back `None`. Located precisely:

- **The server is correct.** `skip_serializing_if = "Option::is_none"` omits the key for `None`
  and emits an explicit `null` for `Some(Value::Null)`. Both dispatchers put
  `"structuredContent":null` on the wire — asserted directly, twice.
- **The collapse is on the way back in.** serde's default `Option<T>` deserializer maps a JSON
  `null` onto `None`, so `CallToolResult`'s own `Deserialize` cannot distinguish "structured
  content is null" from "no structured content".

Handled as: assert presence on the RAW result map (`result_object(&response).get("structuredContent")
== Some(&Value::Null)` — exactly the plan's `Some(null)` vs `None` intent, at the layer where it is
expressible), plus a dedicated tripwire test
`present_null_structured_content_does_not_survive_a_typed_reread` that fails if anyone makes the
deserialization null-preserving without acknowledging it. **Not fixed here** — pre-existing, not a
wire defect, and a change with client-side consequences on BOTH eras. Booked as D-115-04-A.

### 3. `contracts/binding.yaml`'s `structured_value` entry still says `status: planned`

The function now exists and matches the recorded signature exactly
(`pub fn structured_value(value: Value) -> Self`). The file was left untouched, matching the
posture 115-03 took with its five `output_schema_draft_pin` bindings; `tests/phase115_contract_bindings.rs`
passes either way (`planned` is legal on the three Phase 115 equations). Booked as D-115-04-B so
115-10 can flip both sets together.

## Deviations from Plan

### 1. [Rule 1 — measured expectation was wrong] The null assertion moved from the typed result to the raw result map

- **Found during:** Task 2, first run of the new suite
- **Issue:** the plan's `structured_content == Some(Value::Null)` assertion failed with
  `left: None, right: Some(Null)` on BOTH dispatchers
- **Fix:** assert on `result_object(&response).get("structuredContent")`, which expresses
  present-vs-absent exactly as the plan intended; added the serde collapse as a named tripwire
  test rather than absorbing it silently
- **Why not "fixed" in production:** see § Findings 2 — scope boundary (pre-existing, not caused
  by this task) and cross-era client impact
- **Files:** `tests/structured_tool_output.rs`
- **Commit:** `890e3410`

### 2. [Rule 3 — blocking] `initialize_via_core` added to the harness

- **Found during:** Task 2
- **Issue:** the v1 `ServerCore` tests and the non-opted-in half of the anti-vacuity test
  received `-32002` instead of a result
- **Fix:** added `initialize_via_core`, whose rustdoc records the measured gate rule and why only
  the v1 half needs it. No production behaviour was relaxed to accommodate the test — the plan's
  own instruction ("that is a FINDING: record it, do not relax anything")
- **Files:** `tests/common/duplex.rs`
- **Commit:** `890e3410`

### 3. [Operator error — recorded, not hidden] `git checkout --` destroyed uncommitted test work

While reverting negative control A, `git checkout -- tests/structured_tool_output.rs` was used.
Task 2 was not yet committed, so this discarded ~480 lines of uncommitted work rather than
reverting the one-line control. The file was reconstructed and re-verified (20/20 green,
`make lint` clean) before the Task 2 commit; negative control B was then reverted from a
`cp` backup instead. **Pattern for future executors: revert a temporary control from a file COPY,
or commit first — never `git checkout --` on a file carrying uncommitted work.**

### 4. Two clippy fixes in the new harness code

`needless_pass_by_value` on `call_tool_request(.., args: Value, ..)` (the `json!` macro BORROWS
its interpolated values, so `args` was never consumed) and `needless_continue` in
`raw_via_server`'s receive loop. Fixed by building `params` through a `serde_json::Map` — the same
construction and the same reason `tests/common/v2.rs`'s `jsonrpc_envelope` uses — and by an
`if let` loop. Both are `-D warnings` failures under `make lint`, not style preferences.

### Not done, deliberately

- **`contracts/binding.yaml` not edited** (§ Findings 3).
- **`.planning/REQUIREMENTS.md`'s SCHM-02 over-booking not touched** — that is D-115-11-G, owned by
  115-10. This plan's own `requirements-completed: [SCHM-02]` now has runtime evidence behind it,
  which is the thing 115-10 needs in order to reconcile.
- **The plan's `pmat analyze complexity … | jq '.violations[]'` defect did not apply** — this plan
  carries no such verify block (`make quality-gate` is the gate, and PMAT runs in CI per D-07).

## Threat Model Follow-Through

| Threat ID | Disposition | Evidence in this plan |
|---|---|---|
| T-115-13 | mitigate | `structured` byte-unchanged (`git diff` grep → 0); `structured_keeps_its_object_shaped_intent`; `cargo semver-checks` "no semver update required"; `cargo public-api diff` shows ONE addition, zero changes, zero removals |
| T-115-14 | accept | `..._v1_scalar_structured_content_is_unchanged` on both dispatchers; the freeze is stated in the `structured_content` field rustdoc so nobody later "fixes" it; negative control B shows a guard would break v1 too |
| T-115-15 | accept | `..._v2_object_schema_with_scalar_payload_still_returns_a_result` on both dispatchers — warn-only, success payload, value verbatim |
| T-115-35 | mitigate | opt-in + `RequestMeta`-built era signal + `assert_v2_witness`; `structured_output_the_v2_witness_is_load_bearing`; negative control A observed |
| T-115-SC | mitigate | no package installed, no manifest touched — `git diff --stat HEAD~2..HEAD` lists exactly 3 files, none a `Cargo.toml` |

## Known Stubs

None. Every helper added has at least one calling test, and every new test asserts on real
dispatcher output.

## Threat Flags

None. This plan adds no network endpoint, no auth path, no file access and no schema at a trust
boundary; it adds one pure constructor and test-only code.

## Next Steps

- **115-05** — `result_caching_hints` (`ttlMs` / `cacheScope`), the next wave-2 plan.
- **115-10** should pick up D-115-04-A (the present-null serde collapse), D-115-04-B (flip the
  `structured_value` binding to `implemented`, alongside 115-03's five), and D-115-04-C.
- **115-06 / 115-07** can reuse the era-aware seam in `tests/common/duplex.rs` directly. Note its
  hard bound: only `CallTool`, `GetPrompt` and `ReadResource` carry a typed `_meta`, so the list
  methods cannot be driven to v2 through this seam (115-07 already routes around it).

## Self-Check: PASSED

- Files claimed created/modified: all present on disk (`src/types/tools.rs`, `tests/common/duplex.rs`, `tests/structured_tool_output.rs`, `115-04-SUMMARY.md`, `deferred-items.md`).
- Commits claimed: `9c7746c3` and `890e3410` both resolve in `git log`.
- Claimed artifacts spot-checked by content: `pub fn structured_value` in `src/types/tools.rs`, `assert_v2_witness` in `tests/common/duplex.rs`, `structured_output_the_v2_witness_is_load_bearing` in `tests/structured_tool_output.rs`.
