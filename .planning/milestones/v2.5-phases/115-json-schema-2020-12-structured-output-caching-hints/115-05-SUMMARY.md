---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 05
subsystem: api
tags: [caching-hints, cache-scope, ttl-ms, era-v2, cfg-free-projector, serde-lock, schm-03]

# Dependency graph
requires:
  - phase: 115-01
    provides: the pinned `schema/vendored/core-2026-07-28/` artifact — the six `CacheableResult` extenders, the three-element `required` set, and the `ttlMs: {"type":"integer","minimum":0}` measurement that makes `u64` a measured mapping rather than an inference
  - phase: 115-02
    provides: the raw-byte v1 list/read goldens plus the `ttlMs`/`cacheScope` leak guard — the fixtures that prove this plan added slots and not wire bytes
  - phase: 115-04
    provides: the current shape of `src/types/tools.rs` (the `structured_value` sibling), which this plan also edits
  - phase: 115-11
    provides: the `result_caching_hints` contract equation and its four `pmcp::types::caching` bindings, written contract-first — every signature landed exactly as recorded
  - phase: 112
    provides: "`Era` in `src/types/protocol/version.rs` — cfg-free on every target, which is what lets the projector be cfg-free too"
  - phase: 114
    provides: "`inject_v2_result_envelope`, which already supplies the third `required` key (`resultType`); nothing in this phase adds it"
provides:
  - "`pmcp::types::CacheScope` — a CLOSED, `Private`-defaulting enum carrying the spec's `public`/`private` security semantics verbatim, with no `Display` impl and no `#[non_exhaustive]`"
  - "`pmcp::types::DEFAULT_TTL_MS` — `0`, the inert SDK default"
  - "`pmcp::types::caching::project_caching_hints` + `Cacheable` (both `pub(crate)`) in a module that carries NO `cfg`, so BOTH native dispatchers AND the wasm32-only `WasmMcpServer` can reach it"
  - "`Option`-typed `ttl_ms` / `cache_scope` slots with `skip_serializing_if` on all six `CacheableResult` types"
  - "`with_ttl_ms` / `with_cache_scope` builders on the three handler-reachable resource results (6 builders, not 10)"
  - "13 unit tests: 6 projection tests (including the era-less wasm strip path) + 7 serde locks derived from the vendored artifact via `include_str!`"
  - "the reciprocal D-10 rustdoc cross-reference between `TaskV2::ttl_ms` and `types::caching`"
affects: [115-06, 115-07, 115-08, 115-09, 115-10]

# Tech tracking
tech-stack:
  added: []   # no package installed, no manifest touched
  patterns:
    - "Structural mitigation over behavioural: when two dispatchers live under DISJOINT cfgs, the shared logic must live in a cfg-free module or one of them is structurally unable to call it"
    - "`Option` + inject-on-v2 fails CLOSED; non-`Option` + strip-on-v1 fails OPEN. Model a wire-REQUIRED field as `Option` when the alternative leaks across an era boundary"
    - "Serialize the enum for the injected default (`serde_json::to_value(CacheScope::default())`), never a string literal — that is what stops the projection and the enum drifting apart"
    - "A projector that is TOTAL (every input either ensures both keys or removes both keys) has no half-projected state to reason about"
    - "Lock the RUST side against the vendored artifact via `include_str!`, and assert the lookup is non-vacuous, so a schema-shape change fails loudly instead of passing over nothing"

key-files:
  created:
    - src/types/caching.rs
  modified:
    - src/types/mod.rs
    - src/types/protocol/mod.rs
    - src/types/tasks.rs
    - src/types/tools.rs
    - src/types/resources.rs
    - src/types/prompts.rs
    - src/server/core.rs
    - src/server/core_tests.rs
    - src/server/mod.rs
    - src/server/workflow/prompt_handler.rs
    - src/server/simple_resources.rs
    - src/server/wasm_server.rs
    - src/server/wasm_server_tests.rs
    - src/server/traits.rs
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md

key-decisions:
  - "The projector lives in `src/types/caching.rs`, which carries NO `cfg`. `src/server/core.rs` is `cfg(not(target_arch = \"wasm32\"))` and `src/server/wasm_server.rs` is `cfg(target_arch = \"wasm32\")` — disjoint sets, so a projector in either is unreachable from the other. This is T-115-36 mitigated STRUCTURALLY rather than by remembering to call something"
  - "`CacheScope` is NOT `#[non_exhaustive]` (the published union is closed at two values) and has NO `Display` impl (nothing on the wire or in this phase needs one; `Serialize` already produces the wire strings). Both declinations are documented in the type's own rustdoc so they read as decisions"
  - "Six builders, not ten: only `ListResourcesResult`, `ListResourceTemplatesResult` and `ReadResourceResult` reach a `ResourceHandler`. The three dispatcher-built types keep `pub` fields and document the asymmetry instead of carrying API no server author can reach"
  - "`u64` is justified from the GENERATED schema's `{\"type\": \"integer\", \"minimum\": 0}`, never from the TypeScript `number` plus `@minimum 0` — that inference is invalid and was the reasoning the cross-AI review rejected"
  - "The D-10 cross-import tripwire is DECLINED at this layer in favour of reciprocal rustdoc links plus module separation; 115-08 owns the structural assertion"
  - "The projector was NOT wired into any dispatcher — that is 115-06. Doing it here would put an untested call in `wasm_server.rs`, which no native gate compiles"
  - "`contracts/binding.yaml` was NOT edited (bindings stay `status: planned`) — same posture as 115-03 and 115-04; booked as D-115-05-E"

patterns-established:
  - "Pattern: when a plan's grep-shaped acceptance criterion collides with its own prose requirement, satisfy BOTH by rewording the prose, and record the collision rather than silently picking a side"
  - "Pattern: prove a negative control by mutating the tree with a scripted, reversible edit and reverting with `git checkout -- <explicit paths>` — never a blanket reset, and never while the file under test has uncommitted work"
  - "Pattern: in this environment, trust a gate's EXIT CODE over its captured stdout (see D-115-05-F)"

requirements-completed: [SCHM-03]

# Metrics
duration: 105min
completed: 2026-08-01
---

# Phase 115 Plan 05: `CacheableResult` Slots + the cfg-Free Projector Summary

**The six `CacheableResult` types now carry `Option`-typed `ttlMs`/`cacheScope` slots that serialize to nothing when unset — so the v1 golden bytes are untouched — and the projection that will fill or strip them lives in a deliberately `cfg`-free `src/types/caching.rs`, which is the difference between a wasm v1 leak being "a path we must remember to cover" and one that is structurally impossible.**

## Performance

- **Duration:** ~105 min
- **Started:** 2026-08-01T08:35Z (approx)
- **Completed:** 2026-08-01T10:20Z
- **Tasks:** 4
- **Files modified:** 16 (15 source + 1 planning ledger), 1 created

## Accomplishments

- **`src/types/caching.rs` exists, is 668 lines, and carries exactly ONE `#[cfg(...)]` attribute — `#[cfg(test)]`.** Measured, not asserted: `grep -n '#\[cfg(' src/types/caching.rs | grep -v '#\[cfg(test)\]' | wc -l` → `0`. That is the whole T-115-36 mitigation: the module compiles on every target, so `project_caching_hints` is callable from `ServerCore`, from `Server` and from `WasmMcpServer` alike.
- **The projector is TOTAL.** Every input either ensures both keys (`Some(Era::V2)`) or removes both keys (`Some(Era::V1)` **and** `None`) or is the identity (`Cacheable::No`, non-object body). There is no half-projected state, and the `None` arm — the one `WasmMcpServer` will pass, because it carries no `ProtocolContext` — is an active STRIP, not merely "don't add".
- **The injected default is SERIALIZED from the enum, never typed.** `object.entry("cacheScope").or_insert_with(|| serde_json::to_value(CacheScope::default()).expect(...))`. The projector contains zero string literals for the default value, so the default and the enum cannot drift apart.
- **The v1 wire is byte-neutral through this plan, proven by re-running 115-02's fixtures unchanged:** `cargo nextest run --features full -E 'binary(v1_lists_golden)'` → **6 tests run: 6 passed**, including `v1_lists_golden_leak_guard_is_load_bearing`, the test whose job is to fail if `ttlMs`/`cacheScope` ever appear on a v1 response.
- **`cargo semver-checks check-release -p pmcp` reports `0 major` checks failed.** No `constructible_struct_adds_field` violation — all six structs are `#[non_exhaustive]`, exactly as `ReadResourceResult._meta` established in Phase 113. The single `1 minor` failure is a pre-existing, deliberate `#[deprecated]` on `OptimizedSseTransport` (`src/shared/sse_optimized.rs`, last touched 2026-07-27 by Phase 113.1, not in the Phase 115 diff at all).
- **All three negative controls were actually run and actually failed**, with their messages transcribed verbatim below — the assertions are load-bearing, not decorative.
- **Full `make quality-gate` exit 0**, plus both wasm builds green and the four supplementary commands the gate does not cover.

## Task Commits

1. **Task 1: the cfg-free `types::caching` module** — `aeb3f8d2` (feat) — `src/types/caching.rs` (+378), `src/types/mod.rs`, `src/types/protocol/mod.rs`, `src/types/tasks.rs`. 4 files, +391.
2. **Task 2: caching-hint slots on all six result types + build restored** — `807d1f9a` (feat) — the four type modules and the eight server files. 12 files, +510.
3. **Task 3: serde locks against the vendored schema** — `35773031` (test) — `src/types/caching.rs` (+290).
4. **Task 4: full quality gate** — no code change; results transcribed below and committed with this SUMMARY.

## Files Created/Modified

- **`src/types/caching.rs` (NEW, 668 lines)** — module doc (what carries the hints, v2-only/D-11, handler-set + SDK-defaulted/D-08+D-12, the D-10 non-collision with `TaskV2::ttl_ms`, and an explicit "why this module carries no cfg" section that names both dispatcher cfgs and says not to simplify it back into a server module); `CacheScope` with a `# Security` section carrying the spec verbatim; `DEFAULT_TTL_MS`; `pub(crate) Cacheable`; `pub(crate) project_caching_hints`; `mod projection_tests` (6); `mod cacheable_result_serde_locks` (7).
- **`src/types/mod.rs`** — `pub mod caching;` in alphabetical position; a NARROW re-export `pub use caching::{CacheScope, DEFAULT_TTL_MS};` with a comment explaining why the projector and its classification enum stay off the public surface. `grep -c 'Cacheable' src/types/mod.rs` → `0`.
- **`src/types/protocol/mod.rs`** — `pub use super::caching::*;` alongside the `content`/`prompts`/`resources`/`tools` family, so `pmcp::types::protocol::CacheScope` resolves like every sibling wire type; plus `ServerDiscoverResult`'s two slots (+58).
- **`src/types/tasks.rs`** — the reciprocal D-10 note on `TaskV2::ttl_ms` (+7). The cross-reference now exists on BOTH sides, which is what makes it a disambiguation rather than a one-way pointer.
- **`src/types/tools.rs` (+64), `src/types/prompts.rs` (+64)** — `ListToolsResult` / `ListPromptsResult` slots, each documenting the no-builder asymmetry.
- **`src/types/resources.rs` (+262)** — the three handler-reachable results' slots AND their six builders, each with a doctest.
- **`src/server/{core,mod,simple_resources,workflow/prompt_handler,core_tests,wasm_server,wasm_server_tests,traits}.rs`** — 25 struct-literal sites restored. `wasm_server.rs`'s three sites each carry a comment naming 115-06's strip wiring, so the next reader knows the `None` era arm is the point.

## Recorded Measurements

### The vendored facts, re-derived rather than trusted

```
$ python3 -c "import json; d=json.load(open('schema/vendored/core-2026-07-28/schema.json'))['\$defs']['CacheableResult']; ..."
required   ['cacheScope', 'resultType', 'ttlMs']            <- THREE
cacheScope {"enum": ["private", "public"], "type": "string"}
ttlMs      {"minimum": 0, "type": "integer"}
bytes      181474
six        ['CacheableResult', 'DiscoverResult', 'ListPromptsResult',
            'ListResourceTemplatesResult', 'ListResourcesResult',
            'ListToolsResult', 'ReadResourceResult']
```

`resultType` is the third `required` entry and is supplied by Phase 114's `inject_v2_result_envelope`; nothing in this phase adds it to any struct. Serde-lock test 1 states that three-way split explicitly so a reader cannot mistake its absence for a gap.

### Acceptance greps

```
$ grep -n '#\[cfg(' src/types/caching.rs | grep -v '#\[cfg(test)\]' | wc -l      0
$ grep -c 'serve it across authorization contexts' src/types/caching.rs           2
$ grep -c 'a different access token requires a different cache' src/…/caching.rs  2
$ grep -c 'non_exhaustive' src/types/caching.rs                                   0
$ grep -c 'impl .*Display for CacheScope' src/types/caching.rs                    0
$ grep -c 'pub mod caching' src/types/mod.rs                                      1
$ grep -c 'Cacheable' src/types/mod.rs                                            0
$ grep -c '"definitions"' src/types/caching.rs                                    0
$ grep -c 'pub cache_scope' tools.rs resources.rs prompts.rs protocol/mod.rs   1+3+1+1 = 6
$ grep -c 'pub ttl_ms'     tools.rs resources.rs prompts.rs protocol/mod.rs   1+3+1+1 = 6
$ grep -c 'pub fn with_ttl_ms' src/types/resources.rs                             3
$ grep -c 'pub fn with_cache_scope' src/types/resources.rs                        3
$ grep -rn 'with_ttl_ms\|with_cache_scope' tools.rs prompts.rs protocol/mod.rs  (none)
$ grep -c 'ttl_ms: None' <the eight server files>              6+1+6+3+3+3+2+1 = 25
```

### Test counts

```
$ cargo nextest run --lib --features full -E 'test(/types::caching::projection_tests/)'
  Summary  6 tests run: 6 passed, 1782 skipped
$ cargo nextest run --lib --features full -E 'test(/cacheable_result_serde_locks/)'
  Summary  7 tests run: 7 passed, 1788 skipped
$ cargo nextest run --features full -E 'binary(v1_lists_golden)'
  Summary  6 tests run: 6 passed, 0 skipped
$ cargo test --doc --features full
  411 passed (after Task 1)  ->  417 passed (after Task 2)   = exactly the six new builder doctests
```

### The three negative controls (Task 3)

Each was applied with a scripted edit, run, and reverted with `git checkout -- <explicit path>` — never a blanket reset, and never while `src/types/caching.rs` held uncommitted work (Task 3 was committed first, precisely so the controls could not endanger it).

**Control A — rename `pub cache_scope` → `pub cache_scope_v2`** (applied crate-wide with a `\bcache_scope\b` word-boundary regex so the `with_cache_scope` builder name is untouched and the tree still compiles):

```
FAIL types::caching::cacheable_result_serde_locks::rust_field_spellings_match_the_vendored_required_set
panicked at src/types/caching.rs:477:17:
the vendored contract requires `cacheScope` but no Rust field emits it — if the vendored
contract changed, re-run the `## Change protocol` in
schema/vendored/core-2026-07-28/PROVENANCE.md and update the RUST side, never this assertion
```

**Control B — move `#[default]` from `Private` to `Public`:**

```
FAIL types::caching::cacheable_result_serde_locks::the_default_cache_scope_is_private_and_the_default_ttl_is_zero
panicked at src/types/caching.rs:634:9:
assertion `left == right` failed: changing the default to Public is a cross-authorization-context
data leak: a shared gateway would be authorized to serve one caller's response body to another
caller holding a different access token
  left: Public
 right: Private
```

The message contains `data leak`, as the criterion requires.

**Control C — change `ListResourcesResult::ttl_ms` to `Option<f64>`** (builder body cast to `f64` so the control is a TYPE change, not a compile break):

```
FAIL types::caching::cacheable_result_serde_locks::ttl_ms_rust_type_matches_the_vendored_json_schema_type
panicked at src/types/caching.rs:560:9:
ttlMs must serialize as a JSON integer, not a float or a string; got 1.8446744073709552e+19
```

After all three reverts: `cargo nextest run --lib --features full -E 'test(/types::caching/)'` → **13 tests run: 13 passed**.

### `make quality-gate` (Task 4)

Run twice. The first run exited 0 but its redirected log was corrupted by this environment's command proxy (see D-115-05-F); the second, invoked as `/usr/bin/make quality-gate`, produced a faithful 8435-line transcript and also exited 0. Step markers:

```
        PMCP SDK TOYOTA WAY QUALITY GATE
    ✓ Code formatting OK
    ✓ No lint issues
    ✓ widget-runtime built and copied to preview assets
    ✓ Build successful
    ✓ Unit tests passed
    ✓ All doctests passed
    ✓ Property tests passed
    ✓ All examples processed successfully          (89 examples)
    ✓ Integration tests passed
    ✓ All test suites passed (ALWAYS requirements met)
    ✓ pmcp-package fmt/clippy/test OK
    ✓ No vulnerabilities found
    ✓ No technical debt comments
    ✓ No unwrap() calls in production code
    ✓ Fuzz testing completed
    ✅ ALL ALWAYS requirements validated!
    ✓ CB-300: Muda Waste Score: 13.2/100 (Lean)
    ✓ CB-304: Dead Code Percentage: 0.7% (66/5985) [threshold: 15%]
    ✓ CB-1202: Contract Coverage: 2/2 critical keywords covered (100%)
    ✓ CB-1338: No Ghost Bindings: 45 binding(s) verified, 0 ghosts
    ✓ every team-servers binding resolves to a real function
    ✅ ALL TOYOTA WAY QUALITY CHECKS PASSED
    🎯 ALWAYS Requirements Validated
QG2_EXIT=0
```

The four commands the gate does not cover:

```
$ /usr/bin/make wasm-build                                                     WASM_BUILD_EXIT=0
$ cargo build --target wasm32-unknown-unknown --no-default-features \
      --features "wasm,validation"                                        WASM_VALIDATION_EXIT=0
$ cargo nextest run --features full -E 'binary(v1_lists_golden)'    6 tests run: 6 passed
$ cargo semver-checks check-release -p pmcp     223 checks: 222 pass, 1 fail, 0 warn, 30 skip
                                                Summary: 0 major and 1 minor checks failed
```

The one `minor` failure is `type_marked_deprecated` on `OptimizedSseTransport`. It predates this plan: `git log -1 --date=short -- src/shared/sse_optimized.rs` → `dafc77c5 2026-07-27 refactor(113.1): …`, the file carries an intentional `// Why: … deprecated ON PURPOSE (plan 113.1-03, D-01)` header, and it appears nowhere in `git diff --name-only acd23b64..HEAD`. (Evidence is given as file provenance rather than a `git stash` run, because `git stash` is prohibited in this execution environment — the stash stack is shared across worktrees.)

Gate files untouched, as required:

```
$ git diff --stat -- Makefile .github/ deny.toml     (empty)
$ git diff --stat acd23b64..HEAD -- Makefile .github deny.toml   (empty)
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `clippy::items_after_statements` in the new serde-lock module**

- **Found during:** Task 3, on `make lint`
- **Issue:** `const INJECTED_ELSEWHERE: &[&str] = &["resultType"];` was declared mid-function, which `-D warnings` + the pedantic set rejects ("adding items after statements is confusing").
- **Fix:** Hoisted the const to module scope with its own rustdoc, and left an explanatory comment at the use site. No assertion changed.
- **Files modified:** `src/types/caching.rs`
- **Commit:** `35773031`

**2. [Rule 3 - Blocking] The plan's construction-site line numbers had drifted**

- **Found during:** Task 2
- **Issue:** Waves 1-2 edited `core.rs` and `mod.rs`, so the measured line numbers were stale by 3-21 lines (`core.rs` 860→871, 919→930, 993→1004, 1272→1283, 5600→5621; `mod.rs` 2249→2261, 2371→2383, 2462→2474, 5282→5302, 5851→5871).
- **Fix:** Each site was located by its surrounding code rather than by line number, and every file in the plan's list was walked explicitly (including the two orphans the compiler cannot reach). No site was missed.
- **Files modified:** none beyond the intended ones
- **Commit:** `807d1f9a`

### Plan-Text Defects Found (reported, not absorbed)

**3. The `ttl_ms: None` total is 25, not 26.** The criterion says 26; the plan's own `<measured_construction_sites>` block enumerates 6+1+6+3+3+3+2+1 = 25 file:line entries, and the compiler found exactly those 25. Booked as **D-115-05-G**.

**4. Two acceptance criteria contradict the plan's own `<action>` text.** Both were satisfied without weakening either side — by wording the "no builder by design" rustdoc as *"a builder method here would be…"* rather than naming `with_ttl_ms`, and by writing the module doc's dispatcher cfgs as `cfg(target_arch = "wasm32")` without attribute brackets. Booked as **D-115-05-H**.

**5. The environment's command proxy corrupts redirected `make` output.** A genuinely FAILING `make lint` produced a 34-line log with no error text in it; the real failure was visible only through the absolute cargo path. Booked as **D-115-05-F** with the remedy (trust the exit code; use `/usr/bin/make`). Worth flagging because it can make a red gate look green.

### Declinations (decisions, not omissions)

- **No `Display` impl on `CacheScope`.** The `TaskStatus` precedent has one, but nothing on the wire or in this phase needs it: `Serialize` already produces `"public"`/`"private"`, and 115-06 sources the injected default from `serde_json::to_value(CacheScope::default())`. This is the first of the cross-AI review's two public-surface trims. Recorded in the enum's rustdoc.
- **No `#[non_exhaustive]` on `CacheScope`.** The published union is closed at two values; the attribute would force downstream matches to carry an unreachable arm. Fenced operationally by `an_unknown_cache_scope_value_is_rejected`.
- **Six builders, not ten.** The second trim. `ListToolsResult`, `ListPromptsResult` and `ServerDiscoverResult` are dispatcher-built with no handler seam; their fields stay `pub` and their rustdoc states the asymmetry. Consequence booked as **D-115-05-C**.
- **The D-10 cross-import tripwire is declined at this layer** in favour of reciprocal rustdoc links plus module separation. Booked as **D-115-05-D**; 115-08 owns the structural assertion.
- **The projector was not wired into any dispatcher.** That is 115-06. Wiring it here would have put an untested call into `wasm_server.rs`, which no native gate compiles.
- **`contracts/binding.yaml` was not edited.** All four `result_caching_hints` signatures landed exactly as 115-11 recorded them, but the bindings still read `status: planned` — the same posture 115-03 and 115-04 took. Booked as **D-115-05-E**.

## Deferred Items

Eight entries appended to `deferred-items.md` as **D-115-05-A** through **D-115-05-H**. The three the plan's `<output>` block names explicitly:

- **D-115-05-A** — the wasm v1 strip is proven only natively (`no_context_strips_both_keys_which_is_the_wasm_path`) and at compile time (`make wasm-build`), because `wasm_server_tests.rs` does not compile.
- **D-115-05-B** — `src/server/traits.rs` and `src/server/wasm_server_tests.rs` are both orphans; 3 of the 25 insertions live in files no build verifies.
- **D-115-05-C** — no server-builder-level default override for the three dispatcher-built results.

## For 115-06

- `project_caching_hints(&mut Value, Option<Era>, Cacheable)` is ready and unit-tested. Call it with the request's classified `Cacheable` and the resolved era; pass `None` from `WasmMcpServer` (it has no `ProtocolContext`) and the strip arm does the right thing.
- Its three `wasm_server.rs` call sites are already commented in place.
- Only the three resource-side results are handler-settable; the other three will always carry the SDK default on v2.
- `Cacheable` and `project_caching_hints` currently carry `#[allow(dead_code)]` with a `// Why:` comment naming 115-06. **Remove both allows when you wire the calls** — leaving them would mask a future regression where the projector stops being called.

## Threat Model Coverage

| Threat ID | Disposition | How it landed |
|-----------|-------------|---------------|
| T-115-03 | mitigated | Closed two-variant enum, `#[default]` on `Private`, spec semantics verbatim, default serialized from the enum. Fenced by `an_unknown_cache_scope_value_is_rejected` + negative control B. |
| T-115-04 | mitigated | `Option` + `skip_serializing_if`; 115-02's goldens re-run green (6/6) and `unset_hints_emit_no_key_at_all` asserts it at the type level on all six results. |
| T-115-05 | mitigated | Separate modules that never import each other + reciprocal rustdoc on `TaskV2::ttl_ms` and `types::caching`. |
| T-115-16 | mitigated | `include_str!` over `/$defs/CacheableResult`; `the_vendored_schema_lookup_is_not_vacuous` fails if the definition moves or the artifact shrinks below 150 000 bytes. |
| T-115-36 | mitigated | STRUCTURAL: the projector is in a cfg-free module and its `None` arm strips. Proven natively, at compile time, and (in 115-08) structurally. |
| T-115-37 | accepted (bounded) | `u64` justified from the measured `{"type":"integer","minimum":0}`; the absent upper bound is ~584 million years at ms resolution. `ttl_ms_rust_type_matches_the_vendored_json_schema_type` + negative control C are the fence. |
| T-115-SC | mitigated | No package installed, no manifest touched. `git diff --name-only acd23b64..HEAD` contains no `Cargo.toml` change from this plan. |

## Self-Check: PASSED

- `src/types/caching.rs` — FOUND (668 lines)
- `.planning/phases/115-.../115-05-SUMMARY.md` — FOUND (this file)
- `.planning/phases/115-.../deferred-items.md` — FOUND (D-115-05-A..H appended)
- Commit `aeb3f8d2` — FOUND
- Commit `807d1f9a` — FOUND
- Commit `35773031` — FOUND
- No commit in this plan deleted a tracked file (`git diff --diff-filter=D` empty for all three)
