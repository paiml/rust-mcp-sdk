---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 02
subsystem: api
tags: [sep-2640, skills, skills-get, streamable-http, json-rpc, invalid-params, asvs, mcp-protocol]

# Dependency graph
requires:
  - phase: 125-01
    provides: "The proven classifier route — `SKILLS_GET_METHOD` minted a wave early, `InternalClientRequest::SkillsList` as the variant shape, `build_skills_list_response` as the projection shape, `Server.skill_entries` as an `IndexMap` keyed by SKILL.md URI, and `tests/skills_routing.rs` with its `[V1, V2]` fixture builder."
  - phase: 112-v2-discover
    provides: "`build_discover_response` as the shared-projection shape — and, more decisively, the PRECEDENT of DELETING the `ServerCore` wrappers rather than carrying them as dead code."
  - phase: 114-tasks-update
    provides: "The raw-params ordering guarantee and its wire-level proof shape (`malformed_params_from_an_unauthenticated_caller_yield_32003`), copied here as the R-16 gate-ordering test."
  - phase: 115-v2-caching
    provides: "`Cacheable` (with the `No` variant this plan names), the single total `ttlMs`/`cacheScope` projection that makes an absence assertion meaningful, and the `ENVELOPE_SITES` tripwire that forces a new egress to decide."
provides:
  - "`skills/get` answered end to end over streamable HTTP on both POST paths and both eras, with draft-correct `-32602` for every unresolvable-URI and malformed-params case."
  - "A wire-level PROOF that the header/auth pipeline precedes the params error — the property the classifier's raw-params discipline exists to buy."
  - "`build_skills_get_response` as the second shared projection, naming `Cacheable::No` at its call site with the draft's open question as the stated reason."
  - "Direct, transport-independent unit coverage on BOTH projections, in `core.rs`'s own test module."
  - "A measured, tested and guarded `ServerCore` boundary: the resource surface still works, the method reach does not exist, and a source scan fails if the dead delegates are re-added."
  - "A MEASURED correction to D-06's phrasing: the shipped `resources/read` unknown-URI error is `-32603` on the wire (handler-level `-32601` re-wrapped), not `-32601`."
affects: [125-03 validation and limits, 125-04 index.json retirement, 125-05 make test-skills and deferrals]

actuals:
  tokens: 23828   # chars/4 over the realized diff (95,311 chars, a83c6b7a~1..HEAD)
  tasks: 2
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Second method on an established internally-routed seam: reuse the classifier arm shape, the five transport sites, the shared projection and the thin `Server` delegate — and change exactly the deltas that are decisions (error code, cacheability, whether params are read)."
    - "Params-consuming internal route: the classifier stays body-blind, the delegate stays gate-free, and the caller-supplied value is decoded ONCE in the shared projection's served branch."
    - "Caller-supplied identifier lookup as an EXACT map hit with no join/normalize/decode, plus a character-boundary-truncated echo in the error (ASVS V5 + V7)."
    - "Dead-code guard as a source scan whose rustdoc names the precondition a future widener must change first — so the test directs rather than merely blocks."

key-files:
  created: []
  modified:
    - src/types/protocol/mod.rs
    - src/shared/protocol_helpers.rs
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/builder.rs
    - src/server/streamable_http_server.rs
    - tests/skills_routing.rs
    - tests/v2_schema_tripwires.rs

key-decisions:
  - "`skills/get` answers `-32602` for an unknown URI, a reference-file URI, a traversal-shaped URI and every malformed-params shape (D-06) — deliberately diverging from the `-32601` `build_discover_response` uses and from what `SkillsHandler::read` raises for `resources/read`."
  - "MEASURED and recorded: the `resources/read` divergence is LARGER than D-06 states. The handler raises `-32601`, but the dispatch tail re-wraps it, so a caller sees `-32603` with the original code inside the message. Both halves are now pinned by a control test; neither is fixed."
  - "`build_skills_get_response` names `Cacheable::No` at the projection call site. SEP-2640 gives `skills/list` the base list-caching attributes explicitly and leaves the question OPEN for `skills/get`, so pmcp claims nothing and the result carries neither key on either era. Naming rather than omitting makes it a reviewable decision."
  - "`ServerCore` gains NO skills field and NO skills delegate, and the `ServerCoreBuilder::build` call site destructures the 125-01 tuple with a discarding pattern. `ProtocolHandler::handle_request` accepts the typed public `Request` enum, so both would be unreachable dead code — the conclusion Phase 112 reached for `server/discover` and acted on by deleting the wrappers."
  - "The projection takes an already-SERIALIZED `IndexMap<String, Value>` rather than typed entries, for the same feature-gating reason the `skills/list` twin takes a `Vec<Value>`: a build without `feature = \"skills\"` still routes the method and must answer honestly (an empty catalog, in which every URI is a `-32602` miss)."
  - "The URI echo in the `-32602` message is truncated by CHARACTERS, not bytes. A byte-offset slice of an attacker-controlled `String` at a non-UTF-8 boundary panics — that would be a remotely reachable panic dressed up as an ASVS V7 measure."

patterns-established:
  - "A wire-level gate-ordering test needs BOTH halves on ONE server with ONE body: uncredentialed → auth refusal, credentialed → the params error. The first alone passes against a server that refuses everything; the second alone says nothing about order."
  - "Assert the ABSENCE of the caching keys on the v2 framing, not just on v1. `project_caching_hints` is total — every input either ensures both or removes both — so a v2 absence is a real measurement of the disposition passed at the call site rather than a tautology."
  - "A source-scan dead-code guard must filter comment lines when the plan's own instructions put explanatory prose naming the forbidden identifiers at the site; and it must carry an anti-vacuity assertion so the filter cannot silently empty the scan."

requirements-completed: [D-06, D-07]

coverage:
  - id: D1
    description: "A live server answers a `skills/get` POST whose `params.uri` names a registered SKILL.md with a single entry identical in shape to a `skills/list` entry — verbatim frontmatter including the non-required field, the `sha256:`+64-hex digest, the byte-accurate size — carrying `resultType: \"complete\"` and no cursor (D-07)."
    requirement: "D-07"
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#skills_get_returns_the_single_conforming_entry_on_v2"
        status: pass
      - kind: unit
        ref: "src/server/core.rs#build_skills_get_response_answers_a_hit_and_refuses_a_miss_with_invalid_params"
        status: pass
    human_judgment: false
  - id: D2
    description: "`skills/get` on a URI the server does not serve returns `-32602` per the current draft — asserted as the exact numeric code, with `-32601` ruled out separately because that is the specific wrong answer (D-06)."
    requirement: "D-06"
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#skills_get_unknown_uri_returns_invalid_params"
        status: pass
      - kind: integration
        ref: "tests/skills_routing.rs#skills_get_reference_uri_returns_invalid_params"
        status: pass
    human_judgment: false
  - id: D3
    description: "`skills/get` with absent params, non-object params, a missing `uri` key or a non-string `uri` returns `-32602` from the SERVED branch and never panics."
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#skills_get_malformed_params_return_invalid_params"
        status: pass
      - kind: unit
        ref: "src/types/protocol/mod.rs#classify_internal_method_routes_skills_get_with_raw_params"
        status: pass
      - kind: unit
        ref: "src/server/streamable_http_server.rs#classify_http_ingress_routes_skills_get_with_raw_params"
        status: pass
    human_judgment: false
  - id: D4
    description: "The lookup is an EXACT match against the entry map keyed by SKILL.md URI — never joined, normalized or decoded into a path (T-125-06, ASVS V5). `..` segments, appended suffixes and a trailing slash all answer `-32602`, with a positive control proving the canonical spelling DOES resolve."
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#skills_get_traversal_shaped_uri_returns_invalid_params"
        status: pass
      - kind: unit
        ref: "src/server/core.rs#build_skills_get_response_answers_a_hit_and_refuses_a_miss_with_invalid_params"
        status: pass
    human_judgment: false
  - id: D5
    description: "The header and auth pipeline runs BEFORE the -32602 (R-16, T-125-07): against an auth-carrying server the SAME malformed body gets the authentication refusal without credentials and `-32602` with them."
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#skills_get_auth_refusal_precedes_the_params_error"
        status: pass
    human_judgment: false
  - id: D6
    description: "The `skills/get` result carries NEITHER `ttlMs` NOR `cacheScope`, asserted on the wire on a v1 framing AND a v2 framing (R-17) — paired with 125-01's positive assertion that `skills/list` carries both on v2."
    requirement: "D-07"
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#skills_get_result_carries_no_cache_attributes_on_either_era"
        status: pass
      - kind: unit
        ref: "src/server/core.rs#build_skills_get_response_answers_a_hit_and_refuses_a_miss_with_invalid_params"
        status: pass
      - kind: integration
        ref: "tests/v2_schema_tripwires.rs#v2_schema_tripwires_every_envelope_call_site_names_its_cacheability"
        status: pass
    human_judgment: false
  - id: D7
    description: "The `resources/read` unknown-URI divergence is UNCHANGED and now MEASURED rather than assumed: the wire code is -32603 with the handler's -32601 inside the message, and it is not -32602. `resources_read_unknown_uri_method_not_found` in `tests/skills_integration.rs` still passes unchanged."
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#resources_read_unknown_uri_still_returns_method_not_found"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#resources_read_unknown_uri_method_not_found"
        status: pass
    human_judgment: false
  - id: D8
    description: "The two skills methods reach ONLY the high-level `Server` HTTP dispatch. A `ServerCoreBuilder`-built `ServerCore` still serves every skill through `resources/list` and `resources/read` — proving the 125-01 tuple change did not break that path — but answers no skills METHOD, because `ProtocolHandler::handle_request` accepts only the typed public `Request`."
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#server_core_builder_still_serves_skills_as_resources"
        status: pass
    human_judgment: false
  - id: D9
    description: "`ServerCore` carries no `skill_entries` field and no skills delegate, and a source-scan test fails if either is added — because such a delegate would be unreachable dead code, exactly what Phase 112 deleted for `server/discover`."
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#server_core_declares_no_skills_field_or_skills_method"
        status: pass
      - kind: other
        ref: "grep -v '^\\s*//' src/server/core.rs | grep -c 'skill_entries' == 0; sed -n '/impl ServerCore/,$p' src/server/core.rs | grep -v '^\\s*//' | grep -c 'fn handle_skills' == 0"
        status: pass
    human_judgment: false
  - id: D10
    description: "Both projection free functions have direct unit coverage in `src/server/core.rs`'s own test module, beside the `build_discover_response` tests — coverage that depends on no transport."
    verification:
      - kind: unit
        ref: "src/server/core.rs#build_skills_list_response_projects_one_complete_page"
        status: pass
      - kind: unit
        ref: "src/server/core.rs#build_skills_get_response_answers_a_hit_and_refuses_a_miss_with_invalid_params"
        status: pass
      - kind: unit
        ref: "src/server/core.rs#build_skills_get_response_on_an_empty_map_is_invalid_params"
        status: pass
      - kind: unit
        ref: "src/server/core.rs#build_skills_get_response_truncates_the_echoed_uri"
        status: pass
    human_judgment: false
  - id: D11
    description: "`request_is_cacheable` gains no row for either method — cacheability is named at each projection call site (D-07) — and `src/server/core.rs` contains ZERO `\"skills/list\"` / `\"skills/get\"` string literals anywhere, so every dispatch-relevant use resolves through the 125-01 constants."
    verification:
      - kind: other
        ref: "git diff src/server/core.rs shows additions only, no request_is_cacheable arm change; grep -n '\"skills/list\"\\|\"skills/get\"' src/server/core.rs returns nothing"
        status: pass
    human_judgment: false

# Metrics
duration: 33 min
completed: 2026-09-02
status: complete
---

# Phase 125 Plan 02: SEP-2640 `skills/get` + the measured `ServerCore` boundary Summary

**`skills/get` answered end to end over streamable HTTP with draft-correct `-32602` on every unresolvable-URI and malformed-params shape, the auth-before-params ordering proven on the wire rather than argued from the source, and the `ServerCore` method boundary measured, tested in its reachable half and guarded against being "fixed" with dead code.**

## Performance

- **Duration:** 33 min
- **Started:** 2026-09-02T05:53:00Z
- **Completed:** 2026-09-02T06:26:10Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments

- **`skills/get` works, and the error semantics are the draft's, not the neighbourhood's.** A live POST for a registered SKILL.md URI returns one entry byte-shaped like a `skills/list` entry — verbatim frontmatter including the non-required `license`, a wire-verified `sha256:` digest, a byte-accurate size, `resultType: "complete"`, no cursor. An unknown URI, a registered skill's *reference* file, a traversal-shaped URI and four distinct malformed-params shapes all answer exactly `-32602`, each asserted as the numeric code with `-32601` ruled out separately because that is the specific wrong answer sitting on both sides of this function.
- **The gate ordering is PROVEN, not inferred (R-16).** Against a server carrying an auth provider, one malformed body sent twice — once anonymous, once with `Bearer alice` — produces the authentication refusal and then `-32602`. Both halves are required and the summary says why: the first alone would pass against a server that refuses everything; the second alone says nothing about order. The classifier's raw-params discipline is the *mechanism*; this test is the proof, and it is what would catch a future refactor that started deserializing upstream.
- **The `resources/read` divergence turned out to be BIGGER than D-06 recorded, and that was measured rather than assumed.** D-06 says `SkillsHandler::read` returns `-32601`. True at the handler; on the wire the dispatch tail re-wraps it, so a caller sees `-32603` with `-32601` inside the message. Written straight into the control test's rustdoc, because a reader who trusted the phrasing would write a wrong test — and I did, and it failed, which is how this was found.
- **Cacheability is a stated decision on both sides.** `build_skills_list_response` names `Cacheable::Yes`; `build_skills_get_response` names `Cacheable::No`, because the draft gives the listing the base attributes explicitly and leaves the get question open. Both keys are asserted ABSENT on the wire on v1 *and* v2 — and the v2 leg is a real measurement, because the single writer of those keys is total.
- **The `ServerCore` question is settled with the true statement.** The plan's earlier revision claimed builder wire parity the type architecture cannot deliver. What shipped instead: a test that drives a `ServerCoreBuilder`-built core through `raw_via_core` with typed `resources/list` and `resources/read` and gets the skill's URI and body back — the reachable half, and the one that actually regresses if 125-01's tuple change was done wrong — plus a source-scan guard that fails if the dead delegates reappear, whose rustdoc names `ProtocolHandler::handle_request`'s typed-`Request` signature as the thing a future widener must change *first*.
- **Both projections now have transport-independent coverage** in `core.rs`'s own test module, including the URI-echo truncation with a multi-byte leg (a byte-offset slice of an attacker-controlled string is a remotely reachable panic, not a safety measure) and a short-URI control that stops the truncation assertions passing against an implementation that echoed nothing at all.

## Task Commits

1. **Task 1 RED: failing `skills/get` wire tests** — `a83c6b7a` (test) — 8 new tests, all failing with `-32601`; the 8 tracer tests passing.
2. **Task 1 GREEN: route `skills/get` with draft-correct `-32602` semantics** — `0c70268c` (feat)
3. **Task 2: projection coverage and the measured `ServerCore` boundary** — `207761f9` (test)

_No REFACTOR commit: the GREEN implementation needed no cleanup pass, and an empty `refactor(...)` commit would be a claim rather than a change._

## Files Created/Modified

- `src/types/protocol/mod.rs` — `InternalClientRequest::SkillsGet { params }` + its classifier arm; `SKILLS_GET_METHOD` loses the `#[allow(dead_code)]` 125-01 minted it with and its rustdoc records that the route arrived; the `skills/list` classifier test's "no arm yet" assertion FLIPPED to its positive twin; a new `classify_internal_method_routes_skills_get_with_raw_params` covering the garbage-object, non-object, `Null` and three near-miss cases.
- `src/shared/protocol_helpers.rs` — the no-wildcard `InternalClientRequest` alternation in the `tasks/update` routing test gains `SkillsGet`.
- `src/server/core.rs` — `build_skills_get_response` (params decoded in the served branch, exact map hit, `-32602` on miss, truncated echo, `Cacheable::No`), `truncated_uri_for_error` + `SKILLS_GET_URI_ECHO_LIMIT`, a route/no-route rustdoc section on `build_skills_list_response`, and 4 in-module projection tests.
- `src/server/mod.rs` — `Server::handle_skills_get`, a thin gate-free delegate that serializes the entry map at the feature boundary.
- `src/server/builder.rs` — the `ServerCoreBuilder::build` entries discard, rewritten from `.0` to a destructuring pattern and given the Phase-112 precedent alongside the ingress reason.
- `src/server/streamable_http_server.rs` — `HttpIngress::SkillsGet` at all five sites, `assemble_skills_get_fast` / `assemble_skills_get_with_middleware`, three exhaustive-match alternations extended, and two new in-file ingress tests.
- `tests/skills_routing.rs` — 10 new tests (8 `skills/get` wire, 2 `ServerCore`), an extended module-doc property table, a reference-carrying fixture registry, and `#[path]`-included `duplex`.
- `tests/v2_schema_tripwires.rs` — `build_skills_get_response` declared in `ENVELOPE_SITES` with its `Cacheable::No` decision and the reason it differs from its sibling's row.

## Decisions Made

See `key-decisions` in the frontmatter. The three most consequential:

1. **The `Cacheable::No` is named, not omitted.** Omitting it was not an option (the helper takes it with no default, by design), but the *reason* is written at the call site AND in the `ENVELOPE_SITES` row: the draft leaves the question open. A later phase that decides otherwise changes one argument with a stated reason, and the tripwire forces it to say so.

2. **The dead-code guard filters comment lines, and that filter is load-bearing rather than defensive.** This plan's own instructions put explanatory prose at the `ServerCoreBuilder::build` discard site naming the very identifier the guard forbids. An unfiltered scan would turn the plan's documentation instruction into a gate failure. The guard carries two anti-vacuity assertions so the filter cannot silently empty it.

3. **The URI echo truncates by characters.** The obvious `&uri[..96]` compiles, reads as a safety measure, and panics on a multi-byte URI at a non-boundary offset — remotely, on attacker-controlled input. The test carries a 400-emoji leg for exactly that.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `tests/v2_schema_tripwires.rs` `ENVELOPE_SITES` required the new egress**

- **Found during:** Task 1 (anticipated from 125-01's deviation 1, added proactively rather than after the failure)
- **Issue:** `v2_schema_tripwires_every_envelope_call_site_names_its_cacheability` fails with `UNLISTED envelope call site` for any new `inject_v2_result_envelope` caller. The tripwire exists so a new egress must DECIDE `owner` and `cacheable` rather than inherit them by omission. The file is not in the plan's `files_modified`.
- **Fix:** Added an `EnvelopeSite` entry for `build_skills_get_response` stating what it decided and, explicitly, why it differs from the `skills/list` row on exactly one argument.
- **Files modified:** `tests/v2_schema_tripwires.rs`
- **Verification:** `cargo test --all-features --test v2_schema_tripwires -- --test-threads=1` → 13 passed.
- **Committed in:** `0c70268c`

**2. [Rule 3 - Blocking] `src/shared/protocol_helpers.rs` no-wildcard match needed the new variant**

- **Found during:** Task 1 GREEN (compile)
- **Issue:** `test_parse_request_or_internal_routes_tasks_update_with_raw_params` enumerates `InternalClientRequest` without a wildcard, deliberately, so a future internally-routed method cannot be silently absorbed. Adding `SkillsGet` broke it — the guard working as designed. Same file, same reason, as 125-01's deviation 2. The file is not in the plan's `files_modified`.
- **Fix:** Added `SkillsGet { .. }` to the panic-arm alternation, preserving the no-wildcard discipline.
- **Files modified:** `src/shared/protocol_helpers.rs`
- **Verification:** `cargo test -p pmcp --all-features --lib -- --test-threads=1` → 2159 passed.
- **Committed in:** `0c70268c`

**3. [Rule 1 - Bug in the plan's own control test] The `resources/read` divergence control asserted a code that never reaches the wire**

- **Found during:** Task 1 RED (the test failed for the *wrong* reason)
- **Issue:** The plan's action step 5 says to add a control asserting `resources/read` on an unknown URI "still returns its pre-existing `-32601`", quoting D-06. MEASURED: it returns `-32603` — `{"code":-32603,"message":"Protocol error: -32601 - Skill resource not found: ..."}`. D-06's `-32601` is the HANDLER-level code, which `tests/skills_integration.rs:253` pins by calling `ResourceHandler::read` directly; the dispatch tail re-wraps it before it reaches a caller. Writing the plan's assertion literally produces a failing test that indicts the wrong thing.
- **Fix:** The control now asserts BOTH measured facts — wire code `-32603`, and the original `-32601` surviving inside the message — plus the contrast that actually matters, that it is *not* `-32602`. The test's rustdoc states the correction to D-06's phrasing explicitly so the next reader does not repeat the mistake.
- **Files modified:** `tests/skills_routing.rs`
- **Verification:** `resources_read_unknown_uri_still_returns_method_not_found` passes; `resources_read_unknown_uri_method_not_found` in `tests/skills_integration.rs` still passes unchanged (10 passed).
- **Committed in:** `a83c6b7a` (control corrected before the RED commit landed)

**4. [Rule 3 - Blocking] Task 2's `grep -c 'fn build_skills_*_response'` acceptance criterion is unsatisfiable as literally written**

- **Found during:** Task 2 (acceptance-criteria gate)
- **Issue:** The criterion demands each grep return `1`. It returned 2 and 4. The extra hits are the TEST FUNCTION NAMES the plan's own step 3 instructs me to add — `fn build_skills_get_response_answers_a_hit_...` prefix-matches `fn build_skills_get_response`. Renaming them to avoid the collision would break Task 2's own verify command, `cargo test -p pmcp --all-features --lib build_skills`, which selects by that substring. This is the same class the plan explicitly names elsewhere ("An unfiltered count would turn the plan's own documentation instruction into a gate failure") — here it is the test-NAMING instruction.
- **Fix:** Satisfied the criterion's INTENT with a definition-precise check: `grep -c 'fn build_skills_list_response('` → **1**, `grep -c 'fn build_skills_get_response('` → **1**, and a whole-`src/` scan confirming both definitions exist exactly once and only in `core.rs`. No code changed.
- **Files modified:** none
- **Verification:** shown above; `cargo test -p pmcp --all-features --lib build_skills` runs 4 tests, satisfying the "the two direct projection unit tests must run" requirement.
- **Committed in:** n/a (a criterion interpretation, not a code change)

**5. [Rule 2 - Missing Critical] Character-boundary truncation for the error URI echo**

- **Found during:** Task 1 GREEN (implementing the ASVS V7 mitigation)
- **Issue:** The plan says to "truncate or omit" the caller's URI. The obvious byte-offset slice of a `String` PANICS at a non-UTF-8 boundary, and the URI is fully attacker-controlled — that is a remotely reachable panic implemented in the name of a safety control.
- **Fix:** `truncated_uri_for_error` truncates by `chars()`, appends `…` only when it actually truncated, and is covered by a dedicated test with a 400-emoji leg plus a short-URI control proving the echo is not simply empty.
- **Files modified:** `src/server/core.rs`
- **Verification:** `build_skills_get_response_truncates_the_echoed_uri`.
- **Committed in:** `0c70268c` (fn), `207761f9` (test)

---

**Total deviations:** 5 (3 × Rule 3 blocking, 1 × Rule 1 bug, 1 × Rule 2 missing-critical).
**Impact on plan:** No scope creep. Two touched files outside `files_modified`, both because a shipped tripwire or a no-wildcard match demanded the new variant be classified — those guards working as designed, and the same two 125-01 hit. The two semantic departures (deviations 3 and 5) both make the result MORE correct than the plan's literal text: one corrects a factual error in D-06's phrasing that the plan inherited, the other closes a panic the plan's wording would have introduced.

## Issues Encountered

**One measurement hazard, resolved; no code defect.**

The `rtk` proxy again compressed `grep`/`sed` output on source files: an early `grep -n` across four files returned matches from only one, and a `grep -n 'enum Cacheable' -A 40` returned exit 1 against a file that does contain the pattern. Every source scan and acceptance-criteria grep in this plan was therefore run through `/usr/bin/grep` and `/usr/bin/sed` by absolute path, and every cargo command through `/Users/guy/.cargo/bin/cargo`. This is the same class 125-01 recorded for `cargo test` output, now confirmed to affect plain `grep` over source files as well — **not just commands whose output is a test summary.**

The 125-01 note about `cargo test` aborting at the first failing target was honoured: examples were rebuilt (`cargo build --all-features --examples` plus the `pmcp-agent` / `pmcp-team-servers` pair) before the full-suite run, and `tests/docs04_examples_run.rs`'s stale-binary guard did not fire.

## Known Stubs

None introduced by this plan. 125-01's three stubs (reference rows absent from the `resources` manifest, `Skills::entries()`'s unreached `Err` arm, the `debug!`-only exclusion breadcrumb) are all unchanged and all still owned by 125-03.

One boundary worth naming as a boundary rather than a stub, because it is a deliberate limit rather than unfinished work: **a `ServerCoreBuilder`-built `ServerCore` answers no skills METHOD.** It serves every skill through `resources/list` / `resources/read` (asserted), and the method reach is absent because `ProtocolHandler::handle_request` accepts only the typed public `Request`. This is recorded for **125-05**'s deferral list, is guarded against being "fixed" with dead code, and the guard's rustdoc tells a future widener what to change first.

## Threat Flags

None. Every trust boundary this plan crosses is in the plan's own `<threat_model>`, and each mitigation landed and is asserted:

- **T-125-06** (traversal via `params.uri`) — exact `IndexMap` hit, no join/normalize/decode; asserted by `skills_get_traversal_shaped_uri_returns_invalid_params` (with a canonical-URI positive control) and by the miss legs of the core.rs unit test.
- **T-125-07** (gate-ordering inversion) — classifier clones `params` and deserializes nothing; asserted at the unit level in two files AND on the wire by `skills_get_auth_refusal_precedes_the_params_error`.
- **T-125-08** (unbounded URI echo) — character-boundary truncation at 96 chars with an explicit marker; asserted by `build_skills_get_response_truncates_the_echoed_uri`, which also carries the multi-byte panic case.
- **T-125-09** (unbounded params) — accepted as planned; the params are cloned once and read for a single string key, and the transport's body limit is unchanged.
- **T-125-10** (session fixation) — `HttpIngress::SkillsGet` joins the `is_initialize` `false` alternation; asserted by `skills_get_ingress_is_never_an_initialize`.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

**Both SEP-2640 MUST methods are now answered over streamable HTTP.** A conforming host's first two calls no longer get `-32601`.

- **For 125-03 (parallel in wave 2):** every skill fixture this plan authored obeys the phase-wide frontmatter rule — `name` equals the final `/` segment of the resolved URI — including the new reference-carrying registry. Per the plan's wave-2 merge note, **re-run `cargo test --all-features --test skills_routing -- --test-threads=1` after 125-03 lands**; a green run taken before its `validate_names` hard-reject merges does not carry over.
- **For 125-04:** `skills/get` now answers from `Server.skill_entries`, entirely independent of `SkillsHandler`'s `skill://index.json`, so retiring the index touches no skills-method path.
- **For 125-05:** two deferrals to record — (a) D-01 stdio reach, unchanged and still owned; (b) **the `ServerCore` skills-METHOD boundary**, newly measured here, with the widening path written into `server_core_declares_no_skills_field_or_skills_method`'s rustdoc. Also carry forward the D-06 correction: the `resources/read` divergence is `-32603` on the wire, not `-32601`, and the two-level shape should be stated wherever D-06 is restated.
- **Still open for 125-05:** `make quality-gate` continues not to reach this module — every leg pins `--features "full"`, which excludes `skills`. This plan was verified with `--all-features` throughout.

## Self-Check: PASSED

- `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-02-SUMMARY.md` exists on disk.
- `git log --oneline --all | grep a83c6b7a` → found (Task 1 RED, test).
- `git log --oneline --all | grep 0c70268c` → found (Task 1 GREEN, feat).
- `git log --oneline --all | grep 207761f9` → found (Task 2, test).
- Plan `<verification>` re-run at close: `cargo test --all-features -- --test-threads=1` → every target `ok`, **0 failed**, no `failures:` line, doctests (479 passed) reached, so the run completed rather than aborting early; `cargo clippy --all-targets --all-features -- -D warnings` → clean; `cargo build --no-default-features` → exit 0; `cargo build --all-features` → exit 0; `cargo fmt --all -- --check` → clean; `cargo test --all-features --test skills_integration` → 10 passed with `resources_read_unknown_uri_method_not_found` unchanged.
- Plan `<success_criteria>`: all five met — both mandatory methods answered over streamable HTTP ✅; `-32602` on every unresolvable-URI and malformed-params case with the auth gate proven to run first ✅; neither caching key on either era, asserted on the wire ✅; the `ServerCore` boundary measured, tested in its reachable half, guarded, and routed to 125-05's deferral record ✅; no row added to `request_is_cacheable` (core.rs diff is additions only; zero `"skills/*"` literals in the file) ✅.

---
*Phase: 125-sep-2640-conformance-skills-list-skills-get*
*Completed: 2026-09-02*
