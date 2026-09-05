---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 02
type: execute
wave: 2
depends_on: ["125-01"]
files_modified:
  - src/types/protocol/mod.rs
  - src/server/streamable_http_server.rs
  - src/server/core.rs
  - src/server/mod.rs
  - src/server/builder.rs
  - tests/skills_routing.rs
autonomous: true
requirements: [D-06, D-07]
user_setup: []

estimate:
  tokens: 40000
  raw_tokens: 80000
  tasks: 2
  confidence: high

must_haves:
  truths:
    - "A live server answers a `skills/get` POST whose `params.uri` names a registered SKILL.md with a single entry identical in shape to a `skills/list` entry, carrying `resultType: \"complete\"` (D-07)."
    - "`skills/get` on a URI the server does not serve returns JSON-RPC error -32602 (Invalid params), per the current draft — NOT the -32601 the shipped `SkillsHandler::read` returns for `resources/read` (D-06)."
    - "`skills/get` with malformed or absent params returns -32602 from the served branch, after the header and auth pipeline has run, never as a classification-time parse error."
    - "The `skills/get` lookup is an exact-match against the entry map keyed by SKILL.md URI; the caller's URI is never joined, normalized, or manipulated into a path (ASVS V5)."
    - "A server built through `ServerCoreBuilder` answers both methods identically to one built through `Server::builder()` — neither build path returns -32601 while the other succeeds (RESEARCH Pitfall 6)."
    - "`request_is_cacheable` gains no row for either method; cacheability is named at each projection call site (D-07)."
  artifacts:
    - src/types/protocol/mod.rs
    - src/server/streamable_http_server.rs
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/builder.rs
    - tests/skills_routing.rs
  key_links:
    - "`classify_http_ingress` inner `match` -> `HttpIngress::SkillsGet` -> `Server::handle_skills_get` -> `build_skills_get_response`: the same five-site chain the tracer proved for `skills/list`, with the params now actually consumed."
    - "`finalize_skills_resources` -> `ServerCore.skill_entries` (this plan) and -> `Server.skill_entries` (125-01): the ONE function both build paths get entries from. If only one call site is updated, one builder's servers answer -32601 forever."
    - "`build_skills_get_response` error path -> `error_codes::INVALID_PARAMS`, deliberately diverging from the `METHOD_NOT_FOUND` its `build_discover_response` analog uses."
---

<objective>
Expand the tracer's proven path sideways: add `skills/get` on the same classifier
route, and close the twin-site gap so `ServerCoreBuilder`-built servers answer both
methods exactly as `Server::builder()`-built ones do.

Purpose: `skills/get` is the second of the two methods the draft makes MANDATORY
for any server declaring the extension. A server answering only `skills/list` is
still non-conformant. And a phase that wires only one of the two build paths ships
a server whose conformance depends on which builder its author happened to use.

Output: `skills/get` over HTTP with draft-correct `-32602` semantics, and both
build paths carrying entries from one function.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-CONTEXT.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-RESEARCH.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-PATTERNS.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md
</context>

## Artifacts this phase produces

Created in **this plan** (125-02). See 125-01-PLAN.md for the phase-wide table.

| Symbol / artifact | Kind | Location | Visibility |
|---|---|---|---|
| `InternalClientRequest::SkillsGet` | enum variant `{ params: serde_json::Value }` | `src/types/protocol/mod.rs` | `pub(crate)` |
| `HttpIngress::SkillsGet` | enum variant `{ id, params }` | `src/server/streamable_http_server.rs` | private |
| `assemble_skills_get_fast` / `assemble_skills_get_with_middleware` | HTTP response assemblers | `src/server/streamable_http_server.rs` | private |
| `build_skills_get_response` | shared projection free fn | `src/server/core.rs` | `pub(crate)` |
| `Server::handle_skills_get` | thin delegate | `src/server/mod.rs` | `pub(crate)` |
| `ServerCore::handle_skills_list` / `ServerCore::handle_skills_get` | thin delegates | `src/server/core.rs` | `pub(crate)` |
| `ServerCore.skill_entries` | struct field `Arc<IndexMap<String, SkillEntry>>` | `src/server/core.rs` | private |

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: skills/get over HTTP with draft-correct -32602 semantics</name>

  <files>src/types/protocol/mod.rs, src/server/streamable_http_server.rs, src/server/core.rs, src/server/mod.rs, tests/skills_routing.rs</files>

  <read_first>
    - .planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md — what the tracer actually landed, including the exact names of the entry map field and the projection fn.
    - src/types/protocol/mod.rs — the `SKILLS_GET_METHOD` constant and `InternalClientRequest::SkillsList` variant the tracer added, plus the `TasksUpdate` variant's raw-params rustdoc at ~:798-806.
    - src/server/streamable_http_server.rs — the five `HttpIngress::SkillsList` sites the tracer wrote; each needs a `SkillsGet` sibling. Also `assemble_tasks_update_fast` (:3888) and `assemble_tasks_update_with_middleware` (:3942), the params-consuming analog pair, and the `TasksUpdateCall` struct (:3831) showing the argument-bundling convention that avoids `clippy::too_many_arguments`.
    - src/server/core.rs — `build_skills_list_response` as the tracer wrote it, `build_discover_response` (~:2380) for the envelope shape, and the `request_is_cacheable` rustdoc (~:2153-2200) explaining why neither method gets a row.
    - src/server/skills.rs:556-575 — `SkillsHandler::read`, which returns `ErrorCode::METHOD_NOT_FOUND` for an unknown URI. That is the -32601 divergence D-06 records as an out-of-scope observation; do NOT copy it, and do NOT change it.
    - src/server/skills.rs:280-292 — `resolved_path` / `skill_md_uri`, and :321-357 `validate_reference_path`, the registration-time rules the lookup side must not re-open.
    - tests/skills_integration.rs:253 — `resources_read_unknown_uri_method_not_found`, the test that pins the pre-existing -32601 behavior this plan must leave passing.
    - tests/skills_routing.rs — the file to extend.
  </read_first>

  <behavior>
    - `classify_internal_method("skills/get", &garbage_params)` returns `Some(InternalClientRequest::SkillsGet { params })` with `params` byte-identical to the input, including when the input is a non-object or `Value::Null`. The classifier never rejects a body.
    - `classify_internal_method("skills/gets", ...)` returns `None`.
    - A `skills/get` POST whose `params.uri` equals a registered SKILL.md URI returns `result.skill` — a single object with the same `uri`, `frontmatter` and `resources` keys a `skills/list` entry carries — plus `result.resultType == "complete"`.
    - A `skills/get` POST whose `params.uri` names an unregistered URI returns error code -32602.
    - A `skills/get` POST whose `params.uri` names a registered skill's REFERENCE file (not its SKILL.md) returns -32602: the draft says the `uri` MUST be a skill's SKILL.md.
    - A `skills/get` POST with `params` absent, or with `params.uri` a non-string, returns -32602 and does not panic.
    - A `skills/get` POST whose `params.uri` contains `..` path segments or a trailing path suffix appended to a registered URI returns -32602 — the map lookup is exact-match, so no traversal reaches a file.
    - The `skills/get` result carries no pagination cursor key.
  </behavior>

  <action>
Add the second method on the route the tracer proved, consuming its params for the
first time in this phase.

1. `src/types/protocol/mod.rs`: add `InternalClientRequest::SkillsGet { params: serde_json::Value }`
   and its classifier arm keyed on `SKILLS_GET_METHOD`, cloning `params` and
   deserializing nothing. Copy the `TasksUpdate` variant's raw-params rustdoc
   reasoning: a classifier that deserialized would hand an unauthenticated caller a
   params error instead of an auth refusal, inverting the gate ordering. Extend the
   in-module classifier test with `skills/gets` as the near-miss control and a
   non-object params value proving pass-through.

2. `src/server/core.rs`: add `pub(crate) fn build_skills_get_response`, taking the
   entry map, the raw params, the id and the protocol context. Deserialize
   `params.uri` HERE, in the served branch. Four deltas from the
   `build_skills_list_response` sibling the tracer wrote.
   (a) On absent params, a non-object params, a missing `uri` key, or a non-string
   `uri`, return `ServerCore::error_response` with `error_codes::INVALID_PARAMS`
   and a message that does not echo the caller's raw URI unbounded (ASVS V7) —
   truncate or omit it.
   (b) Look the URI up by exact key in the entry map. Never join, normalize,
   percent-decode or otherwise manipulate the caller's string into a path (ASVS
   V5); registration-time `validate_reference_path` already closed the traversal
   surface and the lookup side must not re-open it.
   (c) On a miss, return `error_codes::INVALID_PARAMS` — that is -32602 per the
   draft (D-06). Do NOT use `error_codes::METHOD_NOT_FOUND`, which is what
   `build_discover_response` uses and what the shipped `SkillsHandler::read` returns
   for `resources/read`. Add a rustdoc paragraph recording that the `resources/read`
   -32601 divergence is a known, separately-tracked observation this function
   deliberately does not copy and does not fix.
   (d) On a hit, emit `{"skill": <entry>}` with `ResponseDisposition::Complete` so
   the result carries `resultType: "complete"`. Do NOT name `Cacheable::Yes`: the
   draft explicitly leaves the caching question open for `skills/get` (SEP line
   359). Name the non-cacheable claim at this call site and say in the rustdoc that
   the draft leaves it open, so a later phase can change it with a stated reason.
   Add no row to `request_is_cacheable` for either method — its `match` has no
   wildcard arm and its rustdoc calls such a row a lie about where the claim is
   made.

3. `src/server/mod.rs`: add `pub(crate) fn handle_skills_get` as a thin delegate
   over the shared projection, identical in shape to the `handle_skills_list` the
   tracer wrote. Zero logic; all gates live in `core.rs`.

4. `src/server/streamable_http_server.rs`: add `HttpIngress::SkillsGet { id, params }`
   at all five sites — variant declaration, the `is_initialize` `false` alternation,
   the `classify_http_ingress` fast-reject condition plus inner match arm, the v2
   header-gate alternation with no `method_override`, and the TWO per-path
   response-assembly arms via `assemble_skills_get_fast` and
   `assemble_skills_get_with_middleware`. If either assembler's argument list
   reaches eight, bundle the router inputs into a call struct as `TasksUpdateCall`
   does — that bundling exists because the count was measured against
   `clippy::too_many_arguments`, not anticipated.

5. `tests/skills_routing.rs`: add live-wire tests for every row of this task's
   `<behavior>` block. The unknown-URI, reference-URI, malformed-params and
   traversal-shaped-URI cases must each assert the numeric error code -32602
   explicitly, not merely that an error was returned. Add a control assertion in
   the same file that `resources/read` on an unknown URI still returns its
   pre-existing -32601 — the divergence is deliberate and must be visible as such
   rather than looking like an inconsistency someone should "fix".
  </action>

  <verify>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests" in the output, or fewer than 12 tests reported passed (6 from 125-01 plus this task's cases)</fails_when>
    <automated>cargo test --all-features --test skills_integration -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests" — in particular `resources_read_unknown_uri_method_not_found` must still pass unchanged</fails_when>
    <automated>cargo test -p pmcp --all-features --lib classify_internal_method -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" in the summary line</fails_when>
    <automated>cargo clippy --all-targets --all-features -- -D warnings 2>&amp;1 | tail -5</automated>
    <fails_when>non-zero exit, or any line containing "error:" or "warning:" attributable to src/server/streamable_http_server.rs or src/server/core.rs</fails_when>
  </verify>

  <acceptance_criteria>
    - A test asserts a `skills/get` response for an unregistered URI carries error code exactly -32602.
    - A test asserts a `skills/get` response for a registered skill's reference-file URI carries error code exactly -32602.
    - A test asserts `resources/read` on an unknown URI still carries error code exactly -32601, documenting the divergence as intentional.
    - `grep -c 'skills/list\|skills/get' src/server/core.rs` shows the method strings are referenced only through the `pub(crate)` constants or rustdoc, with no re-typed dispatch literal introduced in a `match` guard.
    - `src/server/core.rs`'s `request_is_cacheable` function body is unchanged: `git diff --stat src/server/core.rs` shows additions, and the `request_is_cacheable` match arms are byte-identical to the pre-plan version.
    - `HttpIngress::SkillsGet` appears at five distinct sites in `src/server/streamable_http_server.rs`: `grep -c 'HttpIngress::SkillsGet' src/server/streamable_http_server.rs` returns at least 5.
    - `cargo build --all-features` exits 0 — the deliberately-exhaustive inner `match` in `classify_http_ingress` compiled, proving no site was skipped.
  </acceptance_criteria>

  <done>
A live server answers `skills/get` with a conforming single entry, and returns
-32602 for an unknown URI, a reference URI, malformed params and a traversal-shaped
URI — while the pre-existing `resources/read` -32601 behavior is untouched and
pinned. Committed.
  </done>
</task>

<task type="auto">
  <name>Task 2: ServerCore twin-site parity — both build paths answer both methods</name>

  <files>src/server/core.rs, src/server/builder.rs, tests/skills_routing.rs</files>

  <read_first>
    - src/server/core.rs:452-512 — the `ServerCore` struct, including the `resources: Option<Arc<dyn ResourceHandler>>` field at ~:475 that the new entries field sits beside.
    - src/server/core.rs:2434-2440 — the twin-site parity rule: ONE shared unit called from BOTH native dispatch sites; `mod.rs` CALLS these helpers and never defines its own.
    - src/server/builder.rs:1426-1455 — `finalize_skills_resources` as 125-01 changed it to return a tuple, and its `ServerCoreBuilder::build` call site at :1356-1360 including the `#[cfg]` / `#[cfg(not)]` pair.
    - src/server/builder.rs:433-525 — `ServerCoreBuilder::skills` and `try_skills`.
    - src/server/mod.rs:5365-5380 — the paired `ServerBuilder::build` call site, so the two stay literally parallel.
    - 125-RESEARCH.md `### Pitfall 6: Two build paths, two places to thread the entries` — the cfg-asymmetry warning: `pub mod skills` is `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` while the `ServerBuilder` methods are plain `#[cfg(feature = "skills")]`. Preserve each site's existing gate; do not harmonize.
  </read_first>

  <action>
Close the gap that makes conformance depend on which builder the server author
chose.

1. `src/server/core.rs`: add a private `skill_entries: Arc<IndexMap<String, SkillEntry>>`
   field on `ServerCore`, placed beside the existing `resources` field and carrying
   a rustdoc line explaining that it is its own field rather than something derived
   from the `ResourceHandler`, because `ComposedResources` may wrap that handler and
   any downcast would silently report "no skills" the day a third composition layer
   appears.

2. `src/server/builder.rs`: populate the new field at the `ServerCoreBuilder::build`
   call site from the tuple `finalize_skills_resources` already returns, mirroring
   the `ServerBuilder::build` site line for line. Update the `#[cfg(not(...))]` arm
   to produce an empty map so the non-skills build keeps compiling.

3. `src/server/core.rs`: add `pub(crate) fn handle_skills_list` and
   `pub(crate) fn handle_skills_get` on `ServerCore` as thin delegates over the same
   `build_skills_list_response` / `build_skills_get_response` free fns the `Server`
   delegates call. Do not define a second projection — the parity rule is that the
   projection exists exactly once and both dispatch sites call it.

4. `tests/skills_routing.rs`: add a parity test that builds the SAME skill registry
   through both `Server::builder()` and `ServerCoreBuilder`, drives `skills/list`
   and `skills/get` against each, and asserts the two results are equal as
   `serde_json::Value` after normalizing the request id. A test that only checks
   both return success would pass while the two diverged in entry content; assert
   equality of the projected value.
  </action>

  <verify>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests", or the parity test is absent from the reported test names</fails_when>
    <automated>cargo test -p pmcp --all-features --lib server::builder -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" in the summary line</fails_when>
    <automated>cargo build --no-default-features 2>&amp;1 | tail -5</automated>
    <fails_when>non-zero exit — proves the `#[cfg(not(...))]` arm of the `ServerCoreBuilder` call site was updated alongside the tuple</fails_when>
    <automated>cargo build --all-features 2>&amp;1 | tail -5</automated>
    <fails_when>non-zero exit, or any line containing "error["</fails_when>
  </verify>

  <acceptance_criteria>
    - A test in `tests/skills_routing.rs` asserts value-level equality between the `skills/list` results produced by a `Server::builder()` server and a `ServerCoreBuilder` server carrying the same registry, and does the same for `skills/get`.
    - `grep -c 'skill_entries' src/server/core.rs` returns at least 3 (field declaration plus the two delegates).
    - `grep -c 'build_skills_list_response\|build_skills_get_response' src/server/core.rs` shows each projection fn is DEFINED once; `grep -c 'fn build_skills_list_response' src/server/core.rs` returns exactly 1 and `grep -c 'fn build_skills_get_response' src/server/core.rs` returns exactly 1.
    - `cargo build --no-default-features` exits 0.
    - `cargo test --all-features -- --test-threads=1` exits 0 with no newly failing test relative to the 125-01 baseline.
  </acceptance_criteria>

  <done>
A `ServerCoreBuilder`-built server and a `Server::builder()`-built server carrying
the same registry return byte-equal `skills/list` and `skills/get` projections, and
the equality is asserted rather than assumed. Committed.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| HTTP client -> `skills/get` `params.uri` | Fully attacker-controlled string used as a lookup key. This is the phase's only caller-supplied identifier. |
| HTTP client -> `skills/get` `params` body | Arbitrary JSON, including non-objects and absent params. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-125-06 | Tampering / Information disclosure | `build_skills_get_response` URI lookup | high | mitigate | Exact-match lookup in the entry `IndexMap` keyed by SKILL.md URI. No path join, no normalization, no percent-decode. Registration-time `validate_reference_path` already rejects `..`, leading `/`, `://` and null bytes; the lookup side does not re-open that. Asserted by the traversal-shaped-URI test returning -32602. |
| T-125-07 | Elevation of privilege | Gate-ordering inversion in the classifier | high | mitigate | `classify_internal_method` clones `params` and deserializes nothing, so a malformed `skills/get` body becomes a -32602 in the served branch AFTER the header/auth pipeline, never a parse error that would leak a params error to an unauthenticated caller. Asserted by the non-object-params classifier test. |
| T-125-08 | Information disclosure | Error message echoing the caller's URI | medium | mitigate | The -32602 message must not echo the raw caller-supplied URI unbounded (ASVS V7); truncate or omit it. |
| T-125-09 | Denial of service | Unbounded `skills/get` params | low | accept | Params are cloned once and read for a single string key; the frame size bound is the transport's existing body limit, unchanged by this plan. |
| T-125-10 | Spoofing / Session fixation | `HttpIngress::SkillsGet` minting a session | high | mitigate | The new variant joins the `is_initialize` `false` alternation, same as its `SkillsList` sibling (ASVS V3). |
</threat_model>

<verification>
- `cargo test --all-features -- --test-threads=1` exits 0 (matches CI).
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0.
- `cargo build --no-default-features` exits 0.
- `cargo test --all-features --test skills_integration -- --test-threads=1` exits 0 with `resources_read_unknown_uri_method_not_found` still passing — this plan changes no existing error code.
</verification>

<success_criteria>
- Both mandatory methods are answered over streamable HTTP.
- `skills/get` uses -32602 for every unresolvable-URI and malformed-params case.
- Both build paths carry entries from one function and return equal projections.
- No row is added to `request_is_cacheable`.
</success_criteria>

<output>
Create `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-02-SUMMARY.md` when done.
</output>
