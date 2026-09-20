---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 06
subsystem: server-dispatch
tags: [caching-hints, schm-03, d-07, d-08, d-11, d-12, v2-projection, wasm, middleware-ordering]
requires:
  - "115-05: src/types/caching.rs (CacheScope, DEFAULT_TTL_MS, Cacheable, project_caching_hints)"
  - "115-02: tests/v1_lists_golden.rs byte fixtures + leak guard"
  - "115-01: schema/vendored/core-2026-07-28 (the six CacheableResult extenders)"
provides:
  - "core::request_is_cacheable — the ONE shared cacheability classifier"
  - "inject_v2_result_envelope's cacheable parameter and both-era projection"
  - "wasm_server::cacheable_result_to_value — the third dispatcher's strip"
affects:
  - "115-08 (source tripwires: wasm call sites + projection/middleware ordering)"
  - "115-10 (books the middleware-ordering limitation as a deferred item)"
tech-stack:
  added: []
  patterns:
    - "capture-before-move: derive a request-only fact while `request` is still in scope"
    - "cfg-free shared projector reached from disjoint cfg islands"
    - "fail-closed classification (catch-all → Cacheable::No)"
key-files:
  created: []
  modified:
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/wasm_server.rs
    - src/server/streamable_http_server.rs
    - src/testing/mod.rs
    - src/types/caching.rs
decisions:
  - "server/discover is wired as the SIXTH cacheable result, deviating from SCHM-03's 'five' — excluding it would ship a knowingly non-conformant v2 server/discover"
  - "request_is_cacheable uses a catch-all No arm: a missing v2 hint is a conformance gap, a spurious hint is a data-leak vector"
  - "The projection is NOT moved after response middleware; the ordering is documented, measured and booked instead"
metrics:
  duration: "~2h55m"
  completed: 2026-08-01
  tasks: 4
  commits: 4
  chokepoint_tests: "16 → 26"
---

# Phase 115 Plan 06: Wire the Caching-Hint Projector into Every Dispatcher — Summary

115-05's cfg-free `project_caching_hints` is now called from all three dispatchers — ensuring
`ttlMs`/`cacheScope` on the v2 wire at the safe inert defaults, and actively STRIPPING them on the
v1 wire including the era-less `WasmMcpServer` that the cross-AI review found leaking.

## What changed

### The native chokepoint (`src/server/core.rs`)

`inject_v2_result_envelope` gained a sixth parameter, `cacheable: Cacheable`, and its body was
restructured so the guards run in a new order:

1. destructure `ResponsePayload::Result(value)`, else return
2. `if !value.is_object() { return; }`
3. `project_caching_hints(value, protocol_context.map(|c| c.era), cacheable)` — **both eras**
4. `if era == Some(Era::V2) { own_reserved_result_fields(...) }` — envelope stays v2-only

Neither `Cacheable` nor the projection logic is defined here; both are imported from
`crate::types::caching`. `grep -c 'pub(crate) enum Cacheable\|fn project_caching_hints'
src/server/core.rs` = **0**, and the wire keys `"ttlMs"` / `"cacheScope"` appear at **0** lines
outside the test module (all 20 occurrences are at lines 5351+, inside `mod tests` which begins at
line 4119). `types::caching` remains the single writer (D-12).

The rustdoc was restated honestly: the old bullet "era != V2 → response left BYTE-IDENTICAL to
today" now reads byte-identical *except that the two v2-only caching-hint keys are STRIPPED if a
handler set them*, with `D-11` named as the severability precedent for Phases 116-119. A new
`# Not the final mutation` section states the measured ordering, the imperative prohibition
(**response middleware MUST NOT mutate `ttlMs`, `cacheScope`, `resultType` or `serverInfo`**), the
`process_response_with_context` signature that makes it possible, why the call was not simply
reordered, and where the deferred item is booked.

### The shared classifier

`pub(crate) fn request_is_cacheable(request: &Request) -> Cacheable` sits beside the injection
helper. **The five `ClientRequest` variant names in the plan were all exact** — no discrepancy to
report:

| Wire method | `ClientRequest` variant (verified at `src/types/protocol/mod.rs:484`) | Result type |
|---|---|---|
| `tools/list` | `ListTools` | `ListToolsResult` |
| `resources/list` | `ListResources` | `ListResourcesResult` |
| `resources/templates/list` | `ListResourceTemplates` | `ListResourceTemplatesResult` |
| `resources/read` | `ReadResource` | `ReadResourceResult` |
| `prompts/list` | `ListPrompts` | `ListPromptsResult` |

The catch-all arm returns `Cacheable::No`, so a new variant fails closed (T-115-17). A
`Request::Server(_)` also returns `No` — it is refused with `-32601` and has no result at all. The
rustdoc states why `server/discover` has no row: it does not ride the `ClientRequest` route, and
`build_discover_response` names its own claim.

### The four native production sites, plus two the plan's map did not list

| Site | Claim | Note |
|---|---|---|
| `core.rs` `build_discover_response` | `Cacheable::Yes` | The measured **sixth** extender |
| `core.rs` `ServerCore::handle_request` | `request_is_cacheable(&request)` | Captured after the `MrtrRound::begin(&request, …)` borrow, before the move into `handle_request_internal`; binding placed outside the `#[cfg(feature = "streamable-http")]` block so it exists on both builds |
| `mod.rs` `handle_tasks_update` | `Cacheable::No` | Comment names `tasks/update`, `UpdateTaskResult`, and D-10 (task `ttlMs` is a LIFETIME, not a cache hint) |
| `mod.rs` `handle_request_with_context` | `request_is_cacheable(&request)` | Captured immediately before the `match request`, whose second arm moves `boxed_req`. `grep -c request_is_cacheable src/server/mod.rs` = **1** — it CALLS, never redefines |
| **`streamable_http_server.rs` `listen_terminal_result_frame`** | `Cacheable::No` | **Not in the plan's map.** `SubscriptionsListenResult` does not extend `CacheableResult` |
| **`testing/mod.rs` reserved-field-registry probe** | `Cacheable::No` | **Not in the plan's map.** The probe measures the reserved-field registry, not the caching projection |

`DispatchEnvelopeClaim` was deliberately NOT extended, as the plan directed.

### The third dispatcher (`src/server/wasm_server.rs`)

A local helper `fn cacheable_result_to_value<T: Serialize>(result: T) -> Result<Value>` serializes
and then calls `crate::types::caching::project_caching_hints(&mut value, None,
Cacheable::Yes)`. `None` is the correct era for this dispatcher, and it selects the STRIP arm.

**The four sites wired, and which serialize handler-returned values:**

| Site | Result type | Handler-returned? |
|---|---|---|
| `handle_list_tools` (now `:197`) | `ListToolsResult` | No — dispatcher-built from `tool_infos` |
| `handle_list_resources` (now `:301`) | `ListResourcesResult` | **Partly.** `WasmResource::list` returns a handler-built `ListResourcesResult`, but this method REBUILDS the result from each provider's `.resources` / `.next_cursor`, so a handler-set hint is already dropped by the rebuild. The strip is belt-and-braces against someone later forwarding the provider's result wholesale |
| `handle_read_resource` (now `:314`) | `ReadResourceResult` | **YES — this is the actual leak.** `WasmResource::read` (`:31`) returns the value and it is serialized VERBATIM with no rebuild. A handler calling `with_cache_scope(CacheScope::Public)` would have put a v2-only key on this era-less dispatcher's v1 wire (T-115-36) |
| `handle_list_prompts` (now `:334`) | `ListPromptsResult` | No — dispatcher-built from `prompt_infos` |

**Non-cacheable sites left exactly as they were**, so the diff shows what this phase considers
cacheable: `handle_initialize` (`:180`), `handle_call_tool` (`:225`, `:240`), `handle_get_prompt`
(`:346`).

`grep -c 'Era' src/server/wasm_server.rs` = **0** — the wasm dispatcher gained no era awareness.

**`resources/templates/list` has no wasm handler at all** — `handle_client_request`'s `_ =>` arm
returns `METHOD_NOT_FOUND` for it. So only four of the six extenders are reachable on this
dispatcher, and `server/discover` is likewise unreachable there.

### Tests (`src/server/core.rs`, `mod inject_v2_result_envelope`)

16 → **26** tests. The ten added cover the chokepoint (not the projector, which
`types::caching::projection_tests` already covers): safe defaults on v2; handler-set hints
surviving verbatim; non-cacheable v2 gaining neither key; v1 gaining neither key; **v1 stripping a
handler-set hint**; `None` context treated as v1 (the exact `WasmMcpServer` combination); errors
untouched on all three era inputs; non-object bodies untouched; the injected scope being
`to_value(CacheScope::default())` rather than a parallel literal; and the middleware-ordering
limitation.

## Verification evidence

| Check | Result |
|---|---|
| `make quality-gate` | **exit 0** (transcript below) |
| `cargo build --features full` | exit 0 |
| `cargo build --no-default-features` | exit 0 |
| `make wasm-build` | exit 0 |
| `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"` | exit 0 |
| `make lint` | exit 0 |
| `make check-todos` | exit 0 |
| `cargo nextest run --lib --features full -E 'test(/inject_v2_result_envelope/)'` | 26 run, 26 passed (pre-change: **16**) |
| `cargo nextest run --features full -E 'binary(v1_lists_golden)'` | **6 tests, 6 passed** — v1 bytes unchanged |
| `cargo nextest run --features full -E 'binary(structured_tool_output)'` | 20 tests, 20 passed — `tools/call` gained no keys |
| `git diff --stat HEAD~3..HEAD -- Makefile .github/ deny.toml` | **EMPTY** — no gate was weakened |

### `make quality-gate` transcript (per-step)

```
        PMCP SDK TOYOTA WAY QUALITY GATE
        Zero Tolerance for Defects
🏭 Jidoka: Stopping the line for quality verification
Checking code formatting...    cargo fmt --all -- --check      ✓ Code formatting OK
Running clippy...              (full, pedantic+nursery+cargo)  ✓ No lint issues
Building...                                                    ✓ Build successful
Doctests...                                                    ✓ All doctests passed
Examples...                                                    ✓ All examples processed successfully
test-all                       test result: ok. 1805 passed; 0 failed; 0 ignored
                                                               ✓ All test suites passed (ALWAYS requirements met)
pmcp-package-gate                                              (passed)
audit                                                          ✓ No vulnerabilities found
unused-deps                                                    (passed)
check-todos                                                    ✓ No technical debt comments
check-unwraps                                                  ✓ No unwrap() calls in production code
validate-always                                                ✅ ALL ALWAYS requirements validated!
purity-check                   PASSED: reader-free + writer-present + zip-permitted + deny-bans-clean
comply                         pmat comply check — all CB-* checks ✓ (incl. CB-1338 45 bindings, 0 ghosts)
        ✅ ALL TOYOTA WAY QUALITY CHECKS PASSED
```

Log: 8464 lines, exit 0.

### pmat complexity, pre vs post

| | Total violations | In `src/server/core.rs` | `inject_v2_result_envelope` |
|---|---|---|---|
| Pre-plan | 6 | **0** | 0 |
| Post-plan | 6 | **0** | 0 |

The six pre-existing violations are all in test/other-crate files
(`crates/mcp-tester/tests/property_tests.rs` ×2, `crates/pmcp-server-toolkit/tests/sql_server_http_example.rs`,
`crates/pmcp-agent/tests/http_sources_mock.rs`, `tests/phase115_contract_bindings.rs`,
`tests/v2_tasks_update_routing.rs`) — none in `src/`, none introduced here. No new violation.

## Negative controls

### Task 2 — `make wasm-build` does NOT catch removal of the wasm projector call

The `project_caching_hints` call was temporarily deleted from `cacheable_result_to_value`, leaving
a bare `serde_json::to_value`. Measured:

```
projector call removed
NEGATIVE CONTROL: make wasm-build exit=0  (0 == the compile gate does NOT catch removal)
```

`grep -c project_caching_hints src/server/wasm_server.rs` dropped from 2 to 1 (the surviving one is
the rustdoc mention). The call was restored and `make wasm-build` re-run at exit 0.

**This is the reason 115-08's source tripwire must exist.** The wasm call site is compile-checked
only; no gate in this repo BEHAVIOURALLY executes it. The three proofs, none sufficient alone, are:
`make wasm-build` (compiles), `types::caching`'s native
`no_context_strips_both_keys_which_is_the_wasm_path` (the arm's behaviour), and 115-08's tripwire
(the call sites exist). All three are named in `cacheable_result_to_value`'s rustdoc.

### Task 3 Control A — the ensure-only design, and it fires

The chokepoint was temporarily rewritten so `project_caching_hints` ran only inside the
`Some(Era::V2)` branch. Both predicted tests failed:

```
FAIL pmcp server::core::tests::inject_v2_result_envelope::no_protocol_context_is_treated_as_v1
FAIL pmcp server::core::tests::inject_v2_result_envelope::v1_strips_a_handler_set_hint
Summary 23/26 tests run: 21 passed, 2 failed
```

Observed messages:

```
panicked at src/server/core.rs:5522: assertion `left == right` failed: an era-less dispatcher
(WasmMcpServer passes exactly this) must STRIP both keys — D-11.
Got {"contents":[],"ttlMs":300000,"cacheScope":"public"}
  left: Object {"contents": Array [], "ttlMs": Number(300000), "cacheScope": String("public")}
 right: Object {"contents": Array []}

panicked at src/server/core.rs:5482: D-11: a v1 wire must NEVER carry a v2 field. An ensure-only
projection would have left this handler-set ttlMs in place.
Got {"resources":[],"nextCursor":null,"ttlMs":300000,"cacheScope":"public"}
```

Reverted; 26/26 green afterwards. Note the 24 unaffected tests passed under the ensure-only variant
— including the six `v1_lists_golden` fixtures, which use handlers that set no hints and therefore
CANNOT catch this class of leak. The two new unit tests are the only fence.

### Task 3 Control B — subsumed permanently, see Deviations

## Deviations from Plan

### Auto-fixed / adjusted

**1. [Rule 3 - Blocking] Two `inject_v2_result_envelope` call sites the plan's map did not list**

- **Found during:** Task 1
- **Issue:** The plan enumerated four native production sites and twenty test sites, all in
  `core.rs` / `mod.rs`. `cargo build --features full` then failed with `E0061` at
  `src/server/streamable_http_server.rs:3106` and `src/testing/mod.rs:505`. The plan's grep had
  only searched two files.
- **Fix:** Both classified and wired explicitly with `Cacheable::No` plus an in-source reason
  (`SubscriptionsListenResult` does not extend `CacheableResult`; the testing probe measures the
  reserved-field registry). Neither is a cacheable result, so no behaviour changed.
- **Files:** `src/server/streamable_http_server.rs`, `src/testing/mod.rs`
- **Commit:** `d64004f5`

**2. [Rule 3 - Blocking] `build_discover_response`'s `Cacheable::Yes` moved from Task 2 into Task 1**

- **Found during:** Task 1
- **Issue:** Task 1's own acceptance criterion requires `make lint` to exit 0. `RUSTFLAGS` in the
  Makefile is `-D warnings`, and with every call site passing `Cacheable::No`, `Cacheable::Yes` had
  no constructor — `warning: variant 'Yes' is never constructed` would have failed the gate at
  Task 1's commit boundary. Committing a tree that does not lint violates CLAUDE.md's
  pre-commit gate.
- **Fix:** The one-line `Cacheable::Yes` at `build_discover_response` (a `core.rs` site, i.e.
  Task 1's own file) landed in Task 1 rather than Task 2. Task 2's acceptance criteria were
  re-verified at Task 2's commit and all hold.
- **Files:** `src/server/core.rs`
- **Commit:** `d64004f5`

**3. [Rule 2 - Missing critical] 115-05's `#[allow(dead_code)]` removed, as its SUMMARY required**

- **Found during:** Task 1
- **Issue:** `Cacheable` and `project_caching_hints` carried `#[allow(dead_code)]` with a `// Why:`
  comment naming this plan. Leaving them would mask a regression where the projector silently stops
  being called.
- **Fix:** Both removed. The dead-code lint is now load-bearing: if a future change drops every
  production call, `-D warnings` fails the build.
- **Files:** `src/types/caching.rs`
- **Commit:** `d64004f5`

### Plan-text defects encountered

**4. [D-115-05-F recurrence] The `pmat ... | jq '.violations[]'` expression in this plan does not work**

Confirmed on pmat 3.15.0 in this checkout: `pmat analyze complexity --format json --max-cognitive 25
| jq '.violations'` returns **`null`**. Violations live at `.summary.violations[]`. The plan carries
this expression in Task 1's acceptance criteria and in `<verification>`. The working path was used
and is recorded above; had the plan text been followed literally it would have reported a false
clean. Same defect the phase already books — flagged again here rather than silently worked around.

**5. Task 3's `300000` literal criterion cannot be met literally without failing `make lint`**

The criterion asks the test module to contain the literal string `300000`.
`clippy::unreadable_literal` is in the pedantic group, which `make lint` enables and does **not**
allow-list, so a bare `300000` is a lint violation. Measured convention in this tree: **zero** bare
6+-digit literals in `src/`, 112 underscore-separated ones — including `src/types/caching.rs`'s own
`300_000`. The tests therefore spell it `300_000` (4 occurrences, lines 5375/5390/5468/5508).
**115-08's tripwire, if it greps for this value, must match `300_000`.**

**6. Task 3's Negative Control B is subsumed by the test itself, permanently**

Control B asks to temporarily switch the middleware from deleting `ttlMs` to deleting `cacheScope`,
confirm the test still describes the same limitation, then revert. The test as written drives
**both** keys in a single run via a `read_with_middleware(deleted_key)` helper, asserting for each
that the deleted key is gone AND that the *other* key still carries the projection. That is
strictly stronger than the temporary control — it proves the test measures ORDERING rather than one
key, it proves the test cannot pass vacuously (a missing projection would fail the second
assertion), and it stays in the tree permanently instead of being reverted. Control B was therefore
not run as a separate temporary edit.

**7. `mod.rs` comment reworded so the `request_is_cacheable` grep is literally 1**

The criterion asks for `grep -c 'request_is_cacheable' src/server/mod.rs` = exactly 1 ("the twin
CALLS it, never redefines it"). The first draft had 2 — one call plus one comment mentioning the
name. The comment now says "the shared classifier in `core.rs`", leaving exactly one code
reference, so the criterion is literally true and a 115-08 tripwire counting occurrences stays
meaningful.

### Threat-model dispositions applied

| Threat | Applied |
|---|---|
| T-115-03 | The injected scope is `serde_json::to_value(CacheScope::default())` inside `types::caching`; `the_injected_scope_is_the_serialization_of_the_enum_default` fences the drift |
| T-115-04 | Projection runs on both eras; `v1_strips_a_handler_set_hint` + `no_protocol_context_is_treated_as_v1` + Control A + the 6 golden fixtures |
| T-115-17 | One shared table, catch-all `No` arm |
| T-115-18 | `cacheable` has no default; all six call sites name it |
| T-115-36 | `cacheable_result_to_value` with era `None` at all four wasm cacheable sites |
| T-115-38 | Accepted, documented, measured and booked (see below) |
| T-115-SC | No package installed, no manifest touched |

## Known limitation (booked for 115-10)

**Response middleware runs AFTER the caching projection and can overwrite it.**

`ServerCore::handle_request` calls `inject_v2_result_envelope` and then
`process_response_with_context(&mut response, &context)`; `src/shared/middleware.rs`'s
`process_response_with_context` takes `response: &mut JSONRPCResponse`, and `AdvancedMiddleware::
on_response_with_context` receives the same `&mut`. A registered response middleware can therefore
add, alter or remove `ttlMs`, `cacheScope`, `resultType` or `serverInfo` after the projection.

This is **accepted, not mitigated**. Moving the projection after the chain would change what
middleware observes about Phase 114's `resultType` / `serverInfo` — a v2 behaviour change outside
SCHM-03's scope. Instead:

- The prohibition is stated imperatively in `inject_v2_result_envelope`'s rustdoc.
- The CURRENT behaviour is MEASURED by
  `response_middleware_still_runs_after_the_projection_and_this_is_a_known_limitation`, which drives
  a real v2 `resources/read` through `ServerCore::handle_request` with a key-deleting middleware and
  asserts the middleware WINS — for either key.
- 115-08 owns the source tripwire fencing the ordering.
- **115-10 must book this as a deferred item with an owner.**

## Notes for downstream plans

- **115-08** needs tripwires for: (a) the four `cacheable_result_to_value` sites in
  `wasm_server.rs` — the Task 2 negative control proves no compile gate catches their removal;
  (b) the `inject_v2_result_envelope` → `process_response_with_context` ordering in
  `ServerCore::handle_request`. If a tripwire greps for the ttl fixture value it must match
  `300_000`, not `300000`.
- **115-10** must book the middleware-ordering deferred item, and separately still owns the ledger
  defects inherited from waves 1-3 (D-115-11-G, D-115-03-A, D-115-04-B, D-115-05-E) — untouched
  here by instruction.
- Four of the six `CacheableResult` extenders are reachable on `WasmMcpServer`.
  `resources/templates/list` and `server/discover` have no wasm handler at all, so a future wasm
  dispatcher that gains either must route it through `cacheable_result_to_value`.

## Known Stubs

None. Every code path added is reached by a passing test or, in the wasm dispatcher's case, by the
compile gate plus the two other proofs named in `cacheable_result_to_value`'s rustdoc.

## Commits

| Task | Commit | Message |
|---|---|---|
| 1 | `d64004f5` | `feat(115-06): project caching hints at the native chokepoint on BOTH eras` |
| 2 | `d4b87130` | `feat(115-06): one shared cacheability classifier, wired at all three dispatchers` |
| 3 | `81555ec6` | `test(115-06): cover the caching projection at the chokepoint, both directions` |
| 4 | `a2242d48` | `docs(115-06): complete caching-hint dispatcher wiring plan` |

## Self-Check: PASSED

All four modified/created files exist on disk; all four commits resolve in `git log --all`.
