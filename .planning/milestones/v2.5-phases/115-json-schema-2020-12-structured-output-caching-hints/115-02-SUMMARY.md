---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 02
subsystem: testing
tags: [golden-fixtures, byte-identity, v1-severability, caching-hints, http, serde]

# Dependency graph
requires:
  - phase: 114-tasks-extension-migration
    provides: "tests/v1_tasks_golden.rs — the raw-byte golden instrument this file restates, and tests/common/v2.rs, the shared real-loopback-HTTP harness"
  - phase: 112-protocol-version-negotiation
    provides: "Era gating and inject_v2_result_envelope, whose v1 early-return is what the resultType/serverInfo half of the leak guard checks"
provides:
  - "tests/v1_lists_golden.rs — five raw-text golden fixtures pinning the v1 wire bytes of tools/list, prompts/list, resources/list, resources/templates/list and resources/read, captured BEFORE any caching-hint field exists"
  - "fn v1_leak_guard(raw) -> Result<(), String> — a callable four-key v1 leak guard (resultType, serverInfo, ttlMs, cacheScope) with a D-11-citing message on the caching-hint branch"
  - "v1_lists_golden_leak_guard_is_load_bearing — the anti-vacuity test that proves the guard fires on each key and accepts a clean frame"
affects: [115-05, 115-06, 116, 117, 118, 119]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pre-change raw-byte capture: pin the wire before the field lands, because the pre-change bytes are unrecoverable afterwards (D-13)"
    - "Callable-guard anti-vacuity: a guard that cannot fire yet is factored into a Result-returning fn so a test can prove it discriminates today"

key-files:
  created:
    - tests/v1_lists_golden.rs
  modified: []

key-decisions:
  - "Register ONE tool and ONE prompt, not two: tools/list and prompts/list iterate std HashMaps whose order is randomized per process, so a two-entry array is not byte-stable. Measured, not assumed — the order flipped 5/8 vs 3/8 across eight runs. Multi-entry coverage is kept via resources/list, whose Vec order this file owns."
  - "Pin resources/templates/list as an EMPTY array: both dispatchers hardcode resource_templates: vec![], and no registration API exists to populate it. The plan's one-entry template fixture is unreachable through the public surface."
  - "Factor the leak guard into fn v1_leak_guard(raw) -> Result<(), String> rather than inline assert!s, so the anti-vacuity test can call it directly with no catch_unwind and no duplicated predicate."
  - "Drop the tasks golden's Frame enum: all five fixtures here are success frames, so V1Golden carries a plain `result: Value`. Carrying an unconstructed Error variant would be dead code."
  - "Keep the name `with_supported_protocol_versions` out of the file entirely — including out of comments — so that a plain grep for it is a working detector of v2 opt-in rather than a hit on prose."

patterns-established:
  - "Capture-then-paste goldens: write the const empty, run, copy the raw frame verbatim out of the wire_break_message failure. Never hand-construct the expected JSON from struct definitions."
  - "Two-sided anti-vacuity: prove a guard REJECTS each offending key AND ACCEPTS a clean input, so a reject-everything guard cannot pass."

requirements-completed: [SCHM-03]

# Metrics
duration: 13min
completed: 2026-08-01
---

# Phase 115 Plan 02: v1 Lists Golden Fixtures Summary

**The five v1 list/read responses are now pinned as raw bytes from a real loopback HTTP round trip against a deliberately not-v2-opted-in server, with a four-key leak guard that is proven to discriminate before it has any real work to do.**

## Performance

- **Duration:** 13 min
- **Started:** 2026-08-01T05:57:12Z
- **Completed:** 2026-08-01T06:10:00Z
- **Tasks:** 2 of 2
- **Files modified:** 1 created, 0 production files touched

## Accomplishments

- Captured the pre-change v1 wire bytes of `tools/list`, `prompts/list`, `resources/list`, `resources/templates/list` and `resources/read` as raw-string literals. Once 115-05 adds `ttlMs` / `cacheScope` to the six `CacheableResult` extenders these bytes are unrecoverable, and this plan is what makes the D-11 severability claim checkable rather than asserted.
- Extended the v1 leak guard from `{resultType, serverInfo}` to `{resultType, serverInfo, ttlMs, cacheScope}` and made it callable, then proved with a dedicated test that it fires on each of the four keys and still accepts a clean frame.
- Two measured findings about the server that the plan did not anticipate (below), both of which changed the fixture's shape rather than being papered over.

## Task Commits

1. **Task 1: Build the harness and capture the five v1 golden frames** — `c6044018` (test)
2. **Task 2: Extend the assertion contract with the SCHM-03 leak guards** — `1b3deea2` (test)

## The five captured frames

All five were captured on 2026-08-01 over real loopback HTTP with `StreamableHttpServerConfig::stateless()` (`enable_json_response: true`, so the raw text IS the JSON-RPC frame, not an SSE-framed copy).

| Fixture | Method | id | Golden bytes |
|---|---|---|---|
| `TOOLS_LIST` | `tools/list` | 1 | 233 |
| `PROMPTS_LIST` | `prompts/list` | 2 | 234 |
| `RESOURCES_LIST` | `resources/list` | 3 | 272 |
| `RESOURCE_TEMPLATES_LIST` | `resources/templates/list` | 4 | 58 |
| `RESOURCES_READ` | `resources/read` | 5 | 134 |

```
{"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"pin_lookup","description":"a fixed tool whose v1 wire shape is pinned by this file","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}]}}
{"jsonrpc":"2.0","id":2,"result":{"prompts":[{"name":"pin_summarize","description":"a fixed prompt whose v1 wire shape is pinned by this file","arguments":[{"name":"topic","description":"the subject to summarize","required":true}]}]}}
{"jsonrpc":"2.0","id":3,"result":{"resources":[{"uri":"pin://fixture/one.txt","name":"one","description":"the first pinned resource","mimeType":"text/plain"},{"uri":"pin://fixture/two.txt","name":"two","description":"the second pinned resource","mimeType":"text/plain"}]}}
{"jsonrpc":"2.0","id":4,"result":{"resourceTemplates":[]}}
{"jsonrpc":"2.0","id":5,"result":{"contents":[{"uri":"pin://fixture/one.txt","mimeType":"text/plain","text":"pinned resource body"}]}}
```

Three properties these literals carry that a structural assert could not:

- **No `nextCursor` key anywhere.** All four list results carry a `skip_serializing_if` cursor, so absence — not `"nextCursor":null` — is what is pinned.
- **`resources/read` element key order is `uri`, `mimeType`, `text`** — *not* the declaration order of `Content::Resource` (`uri`, `text`, `mimeType`). The custom `resource_contents_serde` serializer re-emits the fields and drops the `type` discriminator. Only a byte comparison sees this.
- **`ReadResourceResult._meta` is confirmed absent from the captured bytes**, so `MetaExpectation::Absent` on that fixture is a verified property, not an assumption — the field genuinely exists on the struct (`src/types/resources.rs:369-388`).

## Findings

### Finding 1 — `tools/list` and `prompts/list` are NOT byte-stable with more than one entry

The plan asked for exactly two tools and two prompts. That is not pinnable.

- `Server::handle_list_tools` collects from `self.tool_infos.values()`, and `tool_infos` is a `HashMap<String, ToolInfo>` (`src/server/mod.rs:430`, `:1894`).
- `Server::handle_list_prompts` iterates `self.prompts`, also a `HashMap` (`src/server/mod.rs:434`, `:2234`).

`std::collections::HashMap` randomizes iteration order per process. Measured twice, not assumed:

1. A standalone two-key `HashMap<String, u8>` printed its keys in a different order on 4 of 10 process launches.
2. End to end: a second tool `zz_probe` was temporarily registered and the suite run eight times. The first element of `result.tools` was `zz_probe` on 5 runs and `pin_lookup` on 3. The probe was then reverted (`grep -c zz_probe` → 0).

**Response:** register one tool and one prompt so those arrays are singletons. The comparison was NOT relaxed by a byte — the alternative (sorting, or accepting either order) would have been exactly the papering-over the plan prohibits. Multi-entry array coverage is preserved by `resources/list`, which is served straight from `PinnedResources`'s own fixed `Vec` and is therefore order-stable. This is documented in the file's `# Determinism` module-doc heading so a later reader does not "fix" it by adding a second tool.

### Finding 2 — `resources/templates/list` cannot be made non-empty

Both dispatchers return `resource_templates: vec![]` unconditionally — `src/server/mod.rs:2463` and `src/server/core.rs:994` — and `ResourceHandler` (`src/server/mod.rs:368-382`) has only `read` and `list`, no template leg. There is no registration API to populate it. The plan's "fixed one-entry `ListResourceTemplatesResult`" is therefore unreachable through the public server surface, and the fixture pins the empty array the server actually emits. At 58 bytes it is the thinnest of the five, which makes it the fixture where an injected `ttlMs` / `cacheScope` key is most conspicuous.

### Finding 3 — determinism confirmed

The final suite was run 10 times consecutively at the end of Task 1 and 5 more times after Task 2: 15/15 runs green, `6 tests run: 6 passed, 0 skipped`. No fixture required a `DynamicField`; `NO_DYNAMICS` is empty and every frame is compared verbatim.

## Negative controls

Both were performed, observed and reverted.

**Control 1 (Task 1) — mutate one character inside a golden literal.** `"id":1` → `"id":9` inside `TOOLS_LIST`:

```
FAIL [ 0.023s] pmcp::v1_lists_golden v1_lists_golden_tools_list
thread 'v1_lists_golden_tools_list' panicked at tests/v1_lists_golden.rs:271:5:
assertion `left == right` failed: v1 list/read wire bytes changed. This is a V1 WIRE BREAK,
not a stale fixture — make the change v2-only instead of re-recording the golden.
Raw response was: {"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"pin_lookup",...}]}}
Summary [ 0.024s] 5 tests run: 4 passed, 1 failed, 0 skipped
```

The break named the offending test and carried the wire-break doctrine, not a bare diff. Reverted; suite green.

**Control 2 (Task 2) — neuter `v1_leak_guard` to return `Ok(())` unconditionally:**

```
FAIL [ 0.010s] pmcp::v1_lists_golden v1_lists_golden_leak_guard_is_load_bearing
thread 'v1_lists_golden_leak_guard_is_load_bearing' panicked at tests/v1_lists_golden.rs:723:44:
the guard must REJECT a v1 frame carrying `ttlMs`; it returned Ok for
{"jsonrpc":"2.0","id":1,"result":{"tools":[],"ttlMs":0}}: ()
Summary [ 0.017s] 6 tests run: 5 passed, 1 failed, 0 skipped
```

**The load-bearing observation is the `5 passed`.** With the guard completely disabled, all five wire fixtures stayed GREEN — because `ttlMs` and `cacheScope` do not exist on any result type yet, so their absence is true of a working guard and of a broken one alike. Nothing except the anti-vacuity test could have told anyone the guard was mis-wired, and the first moment they would otherwise have found out is wave 4, when it was supposed to catch a real leak. Reverted; suite green.

## Deviations from Plan

### 1. [Rule 1 — Bug avoided] One tool and one prompt instead of two

- **Found during:** Task 1, before writing the fixture
- **Issue:** The plan specified "exactly two tools" and "exactly two prompts" and simultaneously required a verbatim raw-byte comparison. Both dispatch paths iterate `HashMap`s, so a two-entry array is randomized per process — the fixture as specified would have been a flaky test, which under the Toyota Way zero-defect rule is a defect, not a nuisance.
- **Fix:** Register one tool and one prompt. The plan's own escape hatch (`pin it with a DynamicField`) does not apply: `DynamicField` substitutes string VALUES by key and structurally cannot normalize the order of objects in an array. Reducing to a singleton preserves the strict comparison instead of weakening it, and `resources/list` retains the two-entry array coverage.
- **Files modified:** `tests/v1_lists_golden.rs`
- **Commit:** `c6044018`

### 2. [Rule 3 — Blocking] `resources/templates/list` pinned empty

- **Found during:** Task 1
- **Issue:** The plan called for a fixed one-entry `ListResourceTemplatesResult`. No API exists to register one; both dispatchers hardcode an empty vec.
- **Fix:** Pin the empty array the server actually emits, with a rustdoc on the constant naming both hardcode sites so a future reader does not mistake it for a lazy fixture.
- **Files modified:** `tests/v1_lists_golden.rs`
- **Commit:** `c6044018`

### 3. [Rule 3 — Blocking] `V1Golden` carries `result: Value` instead of a `Frame` enum

- **Found during:** Task 1
- **Issue:** The plan said to copy `v1_tasks_golden.rs`'s `Frame { Result, Error }` enum. Every fixture here is a success frame, so the `Error` variant would never be constructed — dead code under the repo's `RUSTFLAGS = -D warnings`. The same applies to `MetaExpectation::RelatedTaskOnly`.
- **Fix:** `V1Golden.result: Value`, and `MetaExpectation` carries only `Absent`. Both carry a rustdoc explaining that the tasks golden's extra variants exist there because it pins an error frame and a create envelope, and that there is no such fixture here to justify them.
- **Files modified:** `tests/v1_lists_golden.rs`
- **Commit:** `c6044018`

### 4. [Rule 2 — Missing critical] `with_supported_protocol_versions` removed from prose

- **Found during:** Task 1 acceptance check
- **Issue:** The plan's acceptance criterion is `grep -c 'with_supported_protocol_versions' tests/v1_lists_golden.rs` → 0. A rustdoc that mentioned the method by name to explain why it is NOT called returned 1, defeating the grep as a detector.
- **Fix:** Reword the `pinned_server` rustdoc to describe the method without spelling it, and say explicitly in that same doc that the name is kept out of the file so the grep works. The criterion now measures what it was meant to measure.
- **Files modified:** `tests/v1_lists_golden.rs`
- **Commit:** `c6044018`

## Verification

| Check | Result |
|---|---|
| `cargo nextest run --features full -E 'binary(v1_lists_golden)'` | 6 tests run, 6 passed, 0 skipped |
| Exactly 6 tests selected (zero-selection would be a silent pass) | confirmed — Summary line reports 6 |
| Every test name begins with `v1_lists_golden_` | 6/6 |
| `grep -c 'with_supported_protocol_versions'` | 0 |
| `grep -c 'assert_v1_bytes'` | 7 (1 definition + 1 doc link + 5 call sites) |
| All four guard keys present (`resultType`/`serverInfo`/`ttlMs`/`cacheScope`) | 4 / 4 / 9 / 8 occurrences |
| `fn v1_leak_guard` + `v1_lists_golden_leak_guard_is_load_bearing` present | yes |
| Guard failure message contains `D-11` | yes (asserted in-test, per key) |
| Determinism (15 consecutive runs) | 15/15 green |
| `cargo fmt --all -- --check` | pass |
| `make lint` | pass |
| `make check-todos` | pass |
| `git diff --stat -- src/ Cargo.toml` (working tree and index) | EMPTY — zero production bytes changed |

Per the phase commit policy this plan ran the SCOPED gate. The full `make quality-gate` runs once for the phase in 115-10.

## Notes for Future Phases

- **115-05 will turn these red if it gets the era gating wrong, and that is the point.** When `ttlMs` / `cacheScope` land on the six `CacheableResult` extenders, a red fixture here means the field reached a v1 wire. The correct response is to move the emission behind the v2 egress projection (115-06), never to re-record the literal.
- **`v1_leak_guard` becomes load-bearing in wave 4.** It is deliberately vacuous today; the anti-vacuity test is what carries it until then.
- **Do not add a second tool or prompt to this fixture.** Finding 1 documents why, and the module doc restates it. If a future phase makes list ordering deterministic (e.g. by switching to `IndexMap` or sorting), that is itself a v1 wire change and belongs in its own plan with its own golden re-capture rationale.
- **`resources/templates/list` gains a real registration API only if some later phase adds one.** Until then it is the cheapest v1 fixture in the repo and a good canary.

## Threat Model Coverage

| Threat ID | Disposition | How this plan discharged it |
|---|---|---|
| T-115-04 (Tampering, the five v1 list/read responses) | mitigate | Raw-text goldens captured pre-change over real loopback HTTP from a not-opted-in server, plus a callable four-key leak guard with a two-sided anti-vacuity test. Negative control 1 proves a byte change fails a named test. |
| T-115-10 (Repudiation, golden re-recording) | mitigate | The module doc's first heading states that a red golden is a v1 WIRE BREAK and names re-recording as the prohibited response; `wire_break_message` repeats it in the failure text, so the doctrine reaches the person reading CI output, not only the person reading the file. |
| T-115-SC (Tampering, package installs) | mitigate | No package installed, no manifest touched. `git diff -- src/ Cargo.toml` is empty. |

## Self-Check: PASSED

- `tests/v1_lists_golden.rs` — FOUND (742 lines)
- `.planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-02-SUMMARY.md` — FOUND
- Commit `c6044018` — FOUND
- Commit `1b3deea2` — FOUND
