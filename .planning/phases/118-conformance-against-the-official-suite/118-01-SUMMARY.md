---
phase: 118-conformance-against-the-official-suite
plan: 01
subsystem: server-transport
tags: [v2, streamable-http, header-gate, mcp-name, conformance, tasks, contract-first]
requires:
  - "src/types/mrtr.rs::name_bearing_key (the COMBINED routing-name table)"
  - "tests/common/v2.rs (live-HTTP harness)"
  - "pmcp::testing::routing_name_key (public seam onto the pub(crate) tables)"
provides:
  - "v2 header gate requiring Mcp-Name only on name-bearing methods (D-13)"
  - "server-side Mcp-Name validation for tasks/get|update|cancel (D-18)"
  - "Mcp-Name sanitization on non-name-bearing methods (D-20 / T-118-53)"
  - "contracts/mcp-protocol-sdk-v1.yaml::equations.v2_header_gate"
  - "tests/common/v2.rs::v2_headers_for (body-derived conformant Mcp-Name)"
affects:
  - "every v2 POST: a name-less method no longer needs Mcp-Name"
  - "every v2 tasks/* POST: Mcp-Name is now REQUIRED and cross-checked"
  - "clients that emitted Mcp-Name: \"\" for tasks/* must now emit the taskId"
tech-stack:
  added: []
  patterns:
    - "one shared table for both ends of a header cross-check"
    - "literal-oracle contract test alongside a predicate-oracle property test"
    - "byte-level proptest as the sanctioned FUZZ arm for private free functions"
key-files:
  created:
    - tests/v2_mcp_name_name_bearing_only.rs
  modified:
    - src/server/streamable_http_server.rs
    - src/types/mrtr.rs
    - src/types/protocol/error_codes.rs
    - src/shared/streamable_http.rs
    - contracts/mcp-protocol-sdk-v1.yaml
    - crates/mcp-tester/src/tester.rs
    - tests/common/v2.rs
    - tests/common_harness_smoke.rs
    - tests/v2_client.rs
    - tests/v2_required_headers.rs
    - tests/v2_stateless_http.rs
    - tests/v2_subscriptions.rs
    - tests/v2_tasks_era_gates.rs
    - tests/v2_tasks_owner_binding.rs
    - tests/v2_tasks_update_routing.rs
decisions:
  - "D-13 implemented as a straight reversal: no config flag, no feature gate for the old rule"
  - "D-18 implemented by repointing is_name_bearing_method at name_bearing_key (one line + its tests)"
  - "D-20 sanitization carried as String::new(), keeping V2GateOutcome::EnforceOk's shape unchanged"
  - "No binding.yaml entry for v2_header_gate: comply-bindings-check covers only team-servers"
  - "Test harnesses now derive Mcp-Name from the body through the production table, not by hand"
metrics:
  duration: ~4h
  completed: 2026-08-09
  tasks: 3
  commits: 4
  files_changed: 16
  lines: "+1195 / -167"
---

# Phase 118 Plan 01: Relax the v2 `Mcp-Name` Rule to Name-Bearing Methods Summary

`Mcp-Name` is now required exactly where the COMBINED routing-name table says a method
carries a name — reversing the Phase-113 DRIFT-1 presence-on-every-request rule (D-13) and
closing the emitter/validator asymmetry that left `tasks/*` emitted but never validated (D-18).

## Commits

| # | Hash | Type | What |
|---|------|------|------|
| 1 | `ab001de7` | test | RED — failing D-13/D-18 truth-table test over `classify_v2_request` |
| 2 | `123226b9` | feat | GREEN — the relaxed gate, the D-18 delegation, the D-20 sanitization, the rustdoc |
| 3 | `158f238b` | test | property + byte-level arms, and the live-HTTP proof file |
| 4 | `ec73cbc9` | docs | the `v2_header_gate` contract equation and the repo sweep |

## What changed in the gate

`require_three_headers` → **`require_v2_headers`**. `MCP-Protocol-Version` and `Mcp-Method`
are established first; only then does `is_name_bearing_method(&method)` decide whether an
absent `Mcp-Name` is an error. `cross_check_method` and `cross_check_name` are **untouched** —
the strict value comparison is retained exactly where a name exists.

`is_name_bearing_method` now reads `crate::types::mrtr::name_bearing_key` (the COMBINED
table) instead of `logical_name_key` (MRTR-only). That is the whole of D-18: one line, plus
the tests and rustdoc that make it stick.

### The exact new error strings

```rust
const ERR_MISSING_V2_HEADERS: &str =
    "v2 requests must carry Mcp-Method and MCP-Protocol-Version headers";

const ERR_MISSING_MCP_NAME: &str =
    "Mcp-Name header is required: this method carries a routing name";
```

`ERR_MISSING_V2_HEADERS` deliberately does **not** name `Mcp-Name` — a message naming a
conditionally-required header would send an operator looking for the wrong thing.
`require_v2_headers_truth_table` asserts both strings, including
`assert!(!ERR_MISSING_V2_HEADERS.contains("Mcp-Name"))`, so a future collapse back to one
catch-all fails there.

### The D-20 sanitization

When the method is not name-bearing, the carried pair is `(method, String::new())` regardless
of what arrived. `V2GateOutcome::EnforceOk { method, name }` keeps its shape and type, so no
downstream caller changed. The comment at the site names the reason: the pair is echoed back
by `apply_v2_outbound_headers`, and reflecting an unvalidated attacker-supplied string is a
pointless surface (T-118-53). It is observable on the wire — the response's `Mcp-Name` is
empty even when the request sent `Mcp-Name: not-a-real-name`.

## Test functions added

| Function | File | Arm |
|---|---|---|
| `require_v2_headers_truth_table` | `src/server/streamable_http_server.rs` | UNIT (every truth-table row + both error strings) |
| `is_name_bearing_method_matches_the_literal_contract` | same | LITERAL CONTRACT |
| `require_v2_headers_is_exactly_its_truth_table` | same | PROPERTY (typed input space) |
| `v2_header_gate_never_panics_on_arbitrary_bytes` | same | FUZZ (byte-level proptest) |
| `classify_v2_request_requires_mcp_name_only_on_name_bearing_methods` | same | the RED test, kept as the composition-site row set |
| 7 tests in `tests/v2_mcp_name_name_bearing_only.rs` | new file | WIRE (live HTTP) |

### The literal contract list, quoted

```rust
const NAME_BEARING_METHODS: [&str; 6] = [
    "tools/call",
    "prompts/get",
    "resources/read",
    "tasks/get",
    "tasks/update",
    "tasks/cancel",
];

const NAME_LESS_METHODS: [&str; 4] = [
    "tools/list",
    "ping",
    "completion/complete",
    "server/discover",
];
```

It contains **all six** name-bearing methods. Its doc comment states why it exists: the
property test's oracle is `is_name_bearing_method` itself, which by construction cannot
detect a *wrong table* — the predicate would simply agree with itself. This literal is what
pins the table's contents, and negative control (b) below proves it fires.

### The property's oracle is the shared table

```rust
let out = require_v2_headers(&h);
let expected_ok =
    have_version && have_method && (have_name || !is_name_bearing_method(&method));
proptest::prop_assert_eq!(out.is_ok(), expected_ok);

if let Ok((got_method, got_name)) = out {
    proptest::prop_assert_eq!(&got_method, &method);
    if is_name_bearing_method(&got_method) {
        proptest::prop_assert_eq!(&got_name, &name_val);
    } else {
        // The D-20 sanitization: whatever arrived is DISCARDED.
        proptest::prop_assert!(got_name.is_empty());
    }
}
```

No literal array of method names appears in the proptest block. The method *strategy*
(`any_v2_method`) draws from the `NAME_BEARING_METHODS` / `NAME_LESS_METHODS` consts and
mixes in arbitrary `[a-z/]{0,20}` noise, so both classes are reached.

### FUZZ arm — the exemption is withdrawn

Supplied in CLAUDE.md's sanctioned proptest spelling. `HeaderValue::from_bytes` is fed
arbitrary `Vec<u8>` (so non-UTF-8 and RFC 9110 delimiter bytes actually reach
`bounded_header_str`, which `HeaderValue::from_str` could never produce), and
`extract_body_method_and_name` is fed arbitrary raw body bytes. Nothing panics; every
rejection is a structured `Reject { code: HEADER_MISMATCH }`.

**Why no `fuzz/` target:** the gate functions are private free functions, and reaching them
from the `fuzz/` sub-workspace would mean widening pmcp's public API for a test — a worse
trade than the sanctioned proptest spelling. This plan did **not** touch `fuzz/Cargo.toml`
(plan 118-03 owns it in this wave).

### Proptest generated-case counts — MEASURED, not assumed

Instrumented each arm with an `AtomicUsize` + `eprintln!`, ran, read the last line, reverted
the instrumentation:

```
PROPTEST-CASE-COUNT truth_table 256
PROPTEST-CASE-COUNT fuzz_bytes  256
PROPTEST-CASE-COUNT fuzz_bytes  1000     # under PROPTEST_CASES=1000
```

So **256 cases per arm** on the default `cargo test` path (no `proptest.toml` in the repo,
and the Makefile's `PROPTEST_CASES=1000` applies only to `make test-property`'s
`--ignored property_` selection), and the count is env-tunable.

## Negative controls — both executed, quoted, reverted

### (a) Restore the universal-presence rule

Edit: `if !is_name_bearing_method(&method) {` → `if false && !is_name_bearing_method(&method) {`

```
test a_name_less_method_serves_with_absent_empty_or_stray_mcp_name ... FAILED

thread '...' panicked at tests/v2_mcp_name_name_bearing_only.rs:167:5:
assertion `left == right` failed: FAILURE MODE: a v2 tools/list with NO Mcp-Name header did not reach dispatch (status 400).
CONSEQUENCE: this is the Phase-113 DRIFT-1 rule coming back — it rejects the official conformance suite's v2 requests before any handler runs, which fails effectively the whole v2 scored set.
WHAT TO DO: fix the gate per Phase 118 D-13; do not relax this assertion. Body: {"jsonrpc":"2.0","error":{"code":-32020,"message":"Mcp-Name header is required: this method carries a routing name"},"id":1}
  left: 400
 right: 200

test result: FAILED. 6 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

Reverted; the file is byte-identical to the committed state.

### (b) Point `is_name_bearing_method` back at `logical_name_key`

Edit: `name_bearing_key(method).is_some()` → `logical_name_key(method).is_some()`

```
test a_tasks_method_cross_checks_mcp_name_against_the_task_id ... FAILED
test every_tasks_method_rejects_a_missing_mcp_name ... FAILED

thread 'a_tasks_method_cross_checks_mcp_name_against_the_task_id' panicked at tests/v2_mcp_name_name_bearing_only.rs:151:5:
assertion `left == right` failed: FAILURE MODE: a v2 tasks/get whose Mcp-Name disagrees with params.taskId was NOT rejected by the v2 header gate (status 404).
  left: Some(-32601)
 right: Some(-32020)

thread 'every_tasks_method_rejects_a_missing_mcp_name' panicked at ...:151:5:
... a v2 tasks/get with NO Mcp-Name was NOT rejected by the v2 header gate (status 404).
  left: Some(-32601)
 right: Some(-32020)

test result: FAILED. 5 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

And the in-module literal contract test fires too — which is the point of having it:

```
thread 'server::streamable_http_server::tests::is_name_bearing_method_matches_the_literal_contract'
panicked at src/server/streamable_http_server.rs:4711:13:
tasks/get carries a routing name and MUST be name-bearing (D-18)
```

Reverted. **D-18's fix is load-bearing**, not cosmetic.

## Deviations from Plan

### [Rule 1 - Bug] Eleven existing tests encoded the old rule or emitted a non-conformant `Mcp-Name`

**Found during:** Task 1 (GREEN), by running the full suite rather than only the plan's
verification command. The plan anticipated reworking three in-module tests; it did not
anticipate the wire-level fallout.

**Issue and fix, by file:**

| File | Tests | Cause | Fix |
|---|---|---|---|
| `tests/v2_stateless_http.rs` | 1 | asserted `400` for an absent `Mcp-Name` on `tools/list` | renamed `..._rejected` → `..._accepted`, asserts `200` + a `result`, doc names D-13 |
| `tests/v2_required_headers.rs` | 1 | `server_discover_rejects_missing_mcp_name` | renamed to `..._accepts_...`, asserts `200` and that `error` is null |
| `tests/v2_tasks_era_gates.rs` | 1 | harness sent `Mcp-Name: ""` with a `params.taskId` body | harness derives the name from the body |
| `tests/v2_tasks_owner_binding.rs` | 6 | same | same |
| `tests/v2_tasks_update_routing.rs` | 3 | same, plus a fixture whose `taskId` was a NUMBER | see below |

The tasks harnesses were emitting a header that D-18 now (correctly) rejects. Rather than
hand-patching each call site, `tests/common/v2.rs` gained one helper:

```rust
pub fn v2_headers_for(method: &str, params: &Value) -> Vec<(String, String)> {
    let name = pmcp::testing::routing_name_key(method)
        .and_then(|key| params.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default();
    v2_headers(method, name)
}
```

It resolves through the PRODUCTION table via the existing `pmcp::testing::routing_name_key`
seam, so a test can never restate "where the name lives" and drift from the server.

**The one substantive fixture change.** `v2_tasks_update_routing.rs::malformed_update_params`
was `{"taskId": 17, "inputResponses": "not-an-object"}` — `taskId` a NUMBER *on purpose*. Under
D-18 that body can never pass the header gate (a conformant client has no string to put in
`Mcp-Name`, and both an absent and an empty header are rejections), so all three
gate-ORDERING tests would have measured `-32020` instead of the ordering they exist to
protect. The fixture now carries a well-formed string `taskId` with a malformed
`inputResponses`, which still yields the same `-32602` (from
`TASKS_UPDATE_MISSING_INPUT_RESPONSES`) while clearing the header gate. The
"does the refusal echo the caller back into the logs?" assertion was repointed from the
literal `"17"` to a named `MALFORMED_MARKER` const. **The ordering guarantees T-114-63 /
T-114-64 protect are intact** — and in fact strengthened: an unauthenticated caller sending
that body now gets an even earlier refusal that deserializes nothing.

**Commit:** `123226b9`

### [Plan-internal contradiction] Task 1 acceptance criterion vs. Task 1 action item 1

The criterion says `grep -c 'logical_name_key' src/server/streamable_http_server.rs` **returns
0**. Action item 1 of the same task *requires* the new rustdoc to say "before Phase 118 it
resolved through the narrower `logical_name_key`". Both cannot hold.

**Resolution:** satisfied the criterion's intent — **zero CODE references**:

```
$ /usr/bin/grep -v '^[[:space:]]*//' src/server/streamable_http_server.rs \
    | /usr/bin/grep -c 'logical_name_key'
0
```

The 3 remaining occurrences (raw `grep -c` = 3) are all inside `///` doc comments recording
the reversal, which action item 1 mandates. Flagged here so the verifier does not read the raw
count as a miss.

### [Environment, not code] Disk exhaustion twice mid-execution

The volume hit 100% (0 bytes free) during the Task-1 full-suite run and again during negative
control (a) — `cargo` failed with `No space left on device (os error 28)` and the Bash tool
could not even create its own output file. Recovered by removing this worktree's
`target/debug/incremental` (6.1 GB) and later `target/debug/deps`; subsequent builds ran with
`CARGO_INCREMENTAL=0`. No repository content outside this worktree was touched. Recorded
because it presents as spurious build failures (see the project memory note "Disk exhaustion
fakes code regressions") and because it cost a full rebuild.

### [Tooling] `make` output is truncated by the rtk proxy

The first `make quality-gate` capture contained a literal `... (6976 lines truncated)` and no
pass banner, while still reporting `EXIT=0` — i.e. the exit status could not be attributed to
`make` with confidence. Re-ran as `/usr/bin/make quality-gate` (absolute path, bypassing the
hook), which produced both the banner and `EXIT=0`. Consistent with the existing project note
about rtk corrupting `git diff` / `gh pr checks`. **Use the absolute `make` path for gate
evidence.**

### [Tooling] The plan's PMAT jq does not match pmat 3.15.0's schema

The plan's command is
`pmat analyze complexity --format json --max-cognitive 25 | jq '.violations[] | select(.path | startswith("src/"))'`.
In pmat 3.15.0 the violations live at `.summary.violations[]` and the field is `file`, not
`path`, so the plan's query errors with `Cannot iterate over null`. Corrected query and its
result:

```
$ jq -r '.summary.violations[] | select(.file | contains("streamable_http_server"))
         | "\(.rule) \(.value) \(.function)"' cx.json
                     # (no output)

$ jq -r '.summary.violations[].file' cx.json | sort -u
./crates/mcp-tester/tests/property_tests.rs
./crates/pmcp-agent/tests/http_sources_mock.rs
./crates/pmcp-cfn-renderer/tests/support/mod.rs
./crates/pmcp-server-toolkit/tests/sql_server_http_example.rs
./tests/phase115_contract_bindings.rs
./tests/v1_severability_tripwire.rs
./tests/v2_schema_tripwires.rs
./tests/v2_tasks_update_routing.rs
```

**Zero violations in `src/` at all**, and none in `streamable_http_server.rs` — the
restructured `require_v2_headers` needed no decomposition and carries no `#[allow]`. The one
hit in a file this plan touched (`v2_tasks_update_routing.rs`, cog 33) is
`no_source_site_routes_tasks_update_through_the_mrtr_ingress`, a pre-existing scanner test
whose body this plan did not modify, and it is a test file so it is outside the criterion's
`src/` scope.

## Task 3A — the contract equation

`contracts/mcp-protocol-sdk-v1.yaml` gained one `equations.v2_header_gate` entry
(`grep -c 'v2_header_gate'` = **1**) carrying all six required keys — `formula`, `domain`,
`codomain`, `invariants`, `preconditions`, `postconditions` — plus a `lean_theorem`, modelled
on `protocol_version_negotiation`. The comment above it names Phase 118 D-13 and D-18 and
records that it REPLACES the Phase-113 DRIFT-1 rule.

The formula, as committed:

```yaml
    formula: |
      gate: (HeaderMap, meta_is_v2, body_method, body_name) -> EnforceOk | Reject | Passthrough

      name_bearing(m) := name_bearing_key(m) is Some            (the COMBINED table, D-18)
        tools/call, prompts/get -> "name"
        resources/read          -> "uri"
        tasks/get, tasks/update, tasks/cancel -> "taskId"

      era(headers, meta_is_v2):
        (header != v2) && !meta_is_v2 => Passthrough             (a v1 request; gate does not apply)
        (header == v2) != meta_is_v2  => Reject(HEADER_MISMATCH) (header/_meta disagreement)
        (header == v2) && meta_is_v2  => enforce below

      enforce(headers, body_method, body_name):
        MCP-Protocol-Version absent  => Reject(HEADER_MISMATCH)
        Mcp-Method           absent  => Reject(HEADER_MISMATCH)
        Mcp-Method  != body_method   => Reject(HEADER_MISMATCH)
        name_bearing(Mcp-Method):
          Mcp-Name absent                              => Reject(HEADER_MISMATCH)
          decode(Mcp-Name) is None                     => Reject(HEADER_MISMATCH)
          decode(Mcp-Name) != body_name                => Reject(HEADER_MISMATCH)
          otherwise                                    => EnforceOk(Mcp-Method, decode-input)
        !name_bearing(Mcp-Method):
          any Mcp-Name, or none                        => EnforceOk(Mcp-Method, "")

      SANITIZATION: on a method that is not name-bearing the carried name is
      String::new() whatever the client sent, because that pair is echoed back
      outbound by apply_v2_outbound_headers.
```

The five invariants are: (1) `Mcp-Method` + `MCP-Protocol-Version` required on every v2
request; (2) `Mcp-Name` required exactly when `name_bearing_key(method)` is `Some`, read from
one table by both ends; (3) the decoded value equals the body's routing name where it is
required; (4) the carried name is the empty string on non-name-bearing methods; (5) the gate
is total — `Reject`, never a panic, over arbitrary header and body bytes. A sixth records
that a v1 request is `Passthrough`.

**`binding.yaml` verdict — read first, as instructed: NO entry added.** `make
comply-bindings-check` (Makefile ~:843) scans **only** `contracts/team-servers/binding.yaml`
and resolves each `function:` against `crates/pmcp-team-servers/src`. That file's
`target_crate` is `pmcp-team-servers`; it does not cover root-crate functions. An entry for
`require_v2_headers` — a private free function in the root crate — would be a ghost binding
and would fail the gate. The rationale is recorded in the YAML comment above the equation.

**Contract file still parses:** `cargo test --features full --test phase115_contract_bindings`
→ `5 passed; 0 failed`. That test does read `contracts/mcp-protocol-sdk-v1.yaml` at runtime
(`CONTRACT_FILE` at `:90`), so no substitute check was needed and no out-of-band interpreter
was used.

## Task 3B — the sweep

```
$ /usr/bin/grep -rn 'require_three_headers' src/ tests/ examples/ docs/ crates/ .github/
(no matches — exit 1)
```

Six restatements were rewritten:

| file:line (pre-edit) | verdict | action |
|---|---|---|
| `src/types/mrtr.rs:37` | (c) | module doc rewritten: required on name-bearing only, D-13 + D-18 named, client's empty-string emission still valid |
| `src/types/mrtr.rs:239-249` | (c) | `TASK_NAME_BEARING_METHODS`'s "Server-side enforcement is deliberately OFF this phase" → "ON since Phase 118 (D-18)" |
| `src/types/mrtr.rs:355-360` | (c) | `frame_routing_pair`'s "on the SERVER half this widening is inert by design" → now live on both halves |
| `src/shared/streamable_http.rs:1082` | (b→c) | emitter comment kept (still valid) but rewritten: unconditional emission is a SUPERSET, and the `tasks/*` value is now cross-checked |
| `src/shared/streamable_http.rs:2468` | (b→c) | its test doc: the empty value is pinned for backward compatibility, not because it is demanded |
| `tests/v2_client.rs:13,224`, `tests/common_harness_smoke.rs:14` | (c) | renamed the function they cite and restated what the test actually proves |

Four further sites restating "the three *required* headers" were corrected:

| file:line | verdict | action |
|---|---|---|
| `src/types/protocol/error_codes.rs:177` | (c) | `HEADER_MISMATCH` rustdoc now separates the two universally-required headers from the conditionally-required `Mcp-Name` |
| `src/server/streamable_http_server.rs:1235` | (c) | `apply_v2_outbound_headers` → "three v2 *routing* headers", plus a note that `name` is empty for name-less methods (D-20) |
| `tests/v2_subscriptions.rs:190` | (c) | `listen_headers` doc: `Mcp-Name` optional on this name-less method since D-13 |
| `crates/mcp-tester/src/tester.rs:3498` | (b) | `V2HeaderMode::Standard` doc: emitting unconditionally is a valid superset |

### Remaining `Mcp-Name` hits in `src/` and `docs/`, classified

`grep -rn 'Mcp-Name' src/ docs/ | grep -v 'D-13' | grep -v 'D-18'` → **83 hits, 0 in
`docs/`** (`docs/` contains no `Mcp-Name` reference at all, including
`docs/v1-sunset-policy.md`, which was read and does not restate the rule). By file:

| file | hits | verdict |
|---|---|---|
| `src/server/streamable_http_server.rs` | 40 | (a) — the new canonical gate rustdoc, its constants, and the tests asserting them. Includes `:1023` which describes the OLD rule *as history* inside the reversal paragraph |
| `src/types/mrtr.rs` | 19 | (a) — the rewritten module doc and table rustdoc; `:169`, `:235`, `:248`, `:332`, `:359` describe the *emitter*'s contract and the tables, all still true; `:390-460` are the sentinel codec, rule-independent |
| `src/testing/mod.rs` | 8 | (b) — the public codec/table seams. `:136` "whose `Mcp-Name` is the empty string" describes what the client emits, still accurate |
| `src/shared/streamable_http.rs` | 6 | (b) — client emitter and its tests, all rewritten above or rule-independent (`:1794` derives the pair; `:2395` is the Phase-114 `taskId` MUST) |
| `src/client/mod.rs` | 5 | (b) — client-side derivation notes; `:4835` "every request carries … `Mcp-Name` (empty for a name-less method)" is a true statement about *this client* |
| `src/server/task_dispatch.rs` | 2 | (a) — both point at `TASK_NAME_BEARING_METHODS` as the shared table; unchanged and now more true |
| `src/client/subscriptions.rs` | 1 | (b) — the module's own wire contract, `Mcp-Name` empty because the method is not name-bearing |
| `src/shared/transport.rs` | 1 | (a) — names which headers the transport emits; rule-independent |
| `src/types/protocol/error_codes.rs` | 1 | (a) — the line rewritten above (the `D-18` on the following line is what the `grep -v` strips) |

No hit remaining anywhere asserts that `Mcp-Name` must be present on every v2 request.

## Verification

| Command | Result |
|---|---|
| `cargo test --features full --lib server::streamable_http_server` | **66 passed**, 0 failed (`running 66 tests`) |
| `cargo test --features full --test v2_mcp_name_name_bearing_only` | **7 passed**, 0 failed (`running 7 tests`) |
| `cargo test --features full --test phase115_contract_bindings` | 5 passed, 0 failed |
| `RUSTFLAGS="-D warnings" cargo build --features full` | exit 0 |
| `/usr/bin/make comply-bindings-check` | exit 0 — all 4 team-servers bindings resolve |
| `/usr/bin/make quality-gate` | exit 0 — `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` |

Each `<automated>` block ran the cargo invocation under `bash -o pipefail -c '… | tee "$log"'`
and then asserted `^running [1-9][0-9]* tests?$` against the log, so the reported status is
cargo's, not the last pipe stage's (D-19).

### `make comply` report

`make comply` exits 0. `pmat comply check --path .` emits project-level advisories that are
**informational per CLAUDE.md D-07** — it does not react to on-disk binding edits in a single
run, which is exactly why the deterministic `comply-bindings-check` exists and is the
fail-closed half. The advisories are all pre-existing and unrelated to this plan: `CB-121`
lock-poisoning notes in `src/server/progress.rs` / `src/shared/runtime.rs`, `File Health`
(19 files > 2000 lines), `CB-301` Bronze reproducibility, `CB-081` dependency health
(65 duplicate crates), and `CB-130` (CLAUDE.md lacks `pmat query` instructions). `CB-300` Muda
scores 12.9/100 (Lean) and `CB-304` dead code 0.6%. Nothing in the report references the
`v2_header_gate` equation or the files this plan changed.

### Acceptance-criteria greps

```
grep -c 'require_three_headers'  src/server/streamable_http_server.rs   -> 0
grep -c 'require_v2_headers'     src/server/streamable_http_server.rs   -> 18   (>= 2)
grep -c 'name_bearing_key'       src/server/streamable_http_server.rs   -> 5    (>= 1)
grep -v '^\s*//' … | grep -c 'logical_name_key'                         -> 0
grep -v '^\s*//' … | grep -c 'is_name_bearing_method'                   -> 11   (>= 3)
grep -c 'DRIFT-1' -> 4 ; grep -c 'D-13' -> 18 ; grep -c 'D-18' -> 14    (each >= 1)
grep -c '"tools/call"' src/server/streamable_http_server.rs             -> 30
git show HEAD:src/server/streamable_http_server.rs | grep -c '"tools/call"' -> 32
grep -c 'is_name_bearing_method' src/types/mrtr.rs                      -> 3    (>= 1)
grep -c 'proptest!' src/server/streamable_http_server.rs                -> 6    (>= 1)
grep -c 'from_bytes' src/server/streamable_http_server.rs               -> 8    (>= 1)
grep -c 'require_three_headers_needs_all_three' …                       -> 0
grep -c 'sleep' tests/v2_mcp_name_name_bearing_only.rs                  -> 0
grep -cE '2026-07-28|-32020' tests/v2_mcp_name_name_bearing_only.rs     -> 0
grep -c 'v2_header_gate' contracts/mcp-protocol-sdk-v1.yaml             -> 1
all three tasks/* methods present in the wire test                      -> yes
```

The `"tools/call"` literal count went **down** (32 → 30): reworking
`require_three_headers_needs_all_three` into `require_v2_headers_truth_table` routed it
through `NAME_BEARING_METHODS[0]` instead of repeating the literal.

The corrected `logical_name_key` sentence in `src/types/mrtr.rs`, quoted:

> In particular this is **no longer** the server's name-bearing predicate. Until Phase 118,
> `is_name_bearing_method` (in `streamable_http_server.rs`) read THIS function, so the server
> required and cross-checked `Mcp-Name` only for the three MRTR methods while the client
> emitted it for `tasks/*` as well. **Phase 118 D-18** repointed that predicate at
> [`name_bearing_key`], so the emitter and the validator now resolve through one table.

## Not measured here, deliberately

**The post-D-13 v2 conformance scored-set profile is NOT measured by this plan.** The
pre-relaxation 62/91 in RESEARCH is a lower bound, not a baseline, and re-measurement is
**plan 118-05 Task 1's** job. No number is guessed here, and nothing in this SUMMARY should be
read as a claim about how many scenarios now pass.

## Threat Flags

None. Every trust boundary this plan touches is already in the plan's `<threat_model>`:
T-118-01 (spoofing) is mitigated by the retained-and-widened `cross_check_name`; T-118-02
(tampering) by keeping `Mcp-Method` / `MCP-Protocol-Version` mandatory; T-118-04 (DoS) by the
length-bounded `bounded_header_str` plus the byte-level totality proptest; T-118-53
(information disclosure) by the sanitization, asserted on the response header. T-118-03
(a WAF keyed on universal `Mcp-Name` presence silently stopping matching) remains
**accept + document** — it is recorded in the new rustdoc and in the contract equation, and no
runtime warning was added (D-11's no-new-warn stance).

## Known Stubs

None. No hardcoded empty values, placeholder text, or unwired components were introduced.

## Self-Check: PASSED

```
FOUND: tests/v2_mcp_name_name_bearing_only.rs
FOUND: contracts/mcp-protocol-sdk-v1.yaml (equations.v2_header_gate)
FOUND: commit ab001de7  test(118-01) RED
FOUND: commit 123226b9  feat(118-01) the gate
FOUND: commit 158f238b  test(118-01) property + wire
FOUND: commit ec73cbc9  docs(118-01) contract + sweep
```
