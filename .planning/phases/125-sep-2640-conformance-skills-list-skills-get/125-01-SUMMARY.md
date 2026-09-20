---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 01
subsystem: api
tags: [sep-2640, skills, streamable-http, json-rpc, serde_yaml, sha2, semver, mcp-protocol]

# Dependency graph
requires:
  - phase: 80-skills
    provides: "The shipped `skills` feature — `Skill` / `SkillReference` / `Skills`, `SkillsHandler`, `set_skills_capabilities`, and the two builder wiring paths this plan threads entries through."
  - phase: 112-v2-discover
    provides: "The `InternalClientRequest` + `classify_internal_method` + `IngressRequest` routing seam, and `build_discover_response` as the shape a non-`ClientRequest` method's projection takes."
  - phase: 114-tasks-update
    provides: "The `TasksUpdate` variant shape (raw params, no id), the `InternalResponseShape` response tail, and the source-scan tripwire idiom copied for the semver guard."
  - phase: 115-v2-caching
    provides: "`Cacheable`, the single `ttlMs`/`cacheScope` projection, and the `request_is_cacheable` rustdoc that forbids a row for a variant which cannot occur."
provides:
  - "`skills/list` answered end to end over streamable HTTP, on both the fast and the middleware POST paths, on v1 and v2 framings alike."
  - "Public `SkillEntry` / `SkillResourceRef` / `Skills::entries()` — verbatim frontmatter JSON plus a `{uri, digest, size}` manifest — with a written STABLE-vs-unstable boundary."
  - "`InternalClientRequest::SkillsList` and the single-sourced `SKILLS_LIST_METHOD` / `SKILLS_GET_METHOD` constants, ready for 125-02's `skills/get` arm."
  - "`Server.skill_entries` carried as its own field, populated from the one `finalize_skills_resources` call both build paths share."
  - "Crate-private three-way `FrontmatterParse { Absent, Parsed, Invalid }` — the diagnostics substrate 125-03 builds on."
  - "`tests/skills_routing.rs` — the live-wire proof plus five routing guarantees, each with a control that stops it passing vacuously."
affects: [125-02 skills/get, 125-03 validation and limits, 125-04 index.json retirement, 125-05 make test-skills and deferrals]

actuals:
  tokens: 24396   # chars/4 over the realized diff (97,587 chars, 227283c5~1..HEAD)
  tasks: 2
  commits: 2

tech-stack:
  added: ["serde_yaml 0.9 (optional, gated on `skills`; already in Cargo.lock, zero new packages)"]
  patterns:
    - "Internally-routed wire method: crate-private classifier + `pub(crate)` method constant + `HttpIngress` variant at five transport sites + a shared `core.rs` projection + a thin `Server` delegate."
    - "Entry synthesis at build time, carried as its own `Server` field rather than reached by downcasting the `ResourceHandler`."
    - "Three-way parse outcome (`Absent` / `Parsed` / `Invalid`) so a broken input is diagnosable rather than indistinguishable from an absent one."

key-files:
  created:
    - tests/skills_routing.rs
  modified:
    - Cargo.toml
    - src/types/protocol/mod.rs
    - src/shared/protocol_helpers.rs
    - src/server/skills.rs
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/builder.rs
    - src/server/streamable_http_server.rs
    - tests/v2_schema_tripwires.rs

key-decisions:
  - "`parse_frontmatter_value` requires the opening `---` to be the FIRST line of the body, which is stricter than the shipped `parse_frontmatter_description` scanner the plan pointed at. Under the looser 'first `---` in the leading 40 lines' rule an ordinary markdown horizontal rule inside an UNANNOTATED skill opens a phantom block and turns `Absent` into `Invalid` — destroying exactly the three-way distinction R-04 asked for."
  - "`Server::handle_skills_list` hands `core.rs` an already-SERIALIZED `Vec<Value>` rather than typed entries. `SkillEntry` exists only under `feature = \"skills\"` while the classifier that routes the method is ungated, so a featureless build must still answer; serializing at the delegate keeps the shared projection feature-agnostic instead of duplicating the envelope behind a `cfg`."
  - "`ServerCore` deliberately carries NO `skill_entries` field and no handler. Its dispatch accepts only the typed public `Request` enum, which `skills/list` will never appear in, so both would be unreachable dead code. `finalize_skills_resources` returns the entries and the `ServerCoreBuilder` call site discards them with an explicit `.0` plus a comment saying why."
  - "`HttpIngress::SkillsList` carries RAW `params` it does not yet read, with an `#[allow(dead_code)]` stating why: SEP-2640's only `skills/list` parameter is an optional `cursor` and pmcp answers a single complete page (D-11). Keeping the field means a later cursor implementation changes a response assembler rather than re-plumbing the ingress."
  - "`SKILLS_GET_METHOD` is minted here, ahead of its route, under the same single-sourcing rustdoc as `SKILLS_LIST_METHOD` (R-06). Splitting the mint across two waves would mean a second edit to the same rustdoc-governed block — precisely the drift single-sourcing exists to prevent."

patterns-established:
  - "Non-`ClientRequest` wire method: extend the crate-private classifier's fast-reject condition AND its inner exhaustive match. Omitting the fast-reject is a SILENT no-route; omitting the inner arm is only a compile error. Both facts are now written at the site."
  - "Every new `inject_v2_result_envelope` call site must declare its owner and cacheability in `tests/v2_schema_tripwires.rs`'s `ENVELOPE_SITES` table — the tripwire refuses to let a new egress inherit either by omission."
  - "Wire proofs pair an Err assertion with a passing control (`resources/list`), so the assertion cannot hold against a parser that rejects everything."

requirements-completed: [D-01, D-04, D-05, D-07, D-11]

coverage:
  - id: D1
    description: "A built `Server` carrying one frontmatter-bearing skill answers a live `skills/list` POST over streamable HTTP with a conforming entry: verbatim frontmatter, a `sha256:`+64-lowercase-hex digest verified ON THE WIRE, and a byte-accurate `size`."
    requirement: "D-01"
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#skills_list_returns_a_conforming_entry_on_v2"
        status: pass
    human_judgment: false
  - id: D2
    description: "The D-07 era split is observable on the wire in BOTH directions: a v2 `skills/list` result carries `ttlMs` and `cacheScope`, the v1 twin carries neither, and `skills/list` is served on v1 (no era gate) while `server/discover` still answers -32601 there."
    requirement: "D-07"
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#skills_list_answers_on_v1_without_the_caching_attributes"
        status: pass
      - kind: integration
        ref: "tests/skills_routing.rs#skills_list_has_no_era_gate_and_server_discover_still_does"
        status: pass
    human_judgment: false
  - id: D3
    description: "`skills/list` emits `resultType: \"complete\"`, returns every entry in ONE response with no `nextCursor` key, and an EMPTY registry answers with `skills: []` rather than -32601 (D-11)."
    requirement: "D-11"
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#empty_registry_answers_skills_list_with_an_empty_array"
        status: pass
      - kind: integration
        ref: "tests/skills_routing.rs#skills_list_returns_a_conforming_entry_on_v2"
        status: pass
    human_judgment: false
  - id: D4
    description: "Public `Skills::entries()` / `SkillEntry` / `SkillResourceRef` API — `#[non_exhaustive]` with private fields plus accessors, entry order equal to registration order, SKILL.md first in each manifest, and a rustdoc stating the STABLE-vs-unstable boundary plus the verbatim-frontmatter disclosure warning."
    verification:
      - kind: unit
        ref: "src/server/skills.rs#entries_synthesizes_one_conforming_entry"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#entries_excludes_frontmatter_less_and_malformed_skills"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#entry_serialization_emits_exactly_the_wire_keys"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#entries_honor_with_path"
        status: pass
      - kind: other
        ref: "cargo test --doc --all-features skills -- --test-threads=1 (Skills::entries doctest)"
        status: pass
    human_judgment: false
  - id: D5
    description: "The YAML frontmatter parse is reachable through exactly ONE crate-private function (D-04) that distinguishes THREE outcomes — absent, parsed, present-but-invalid — with LF and CRLF bodies producing identical objects."
    requirement: "D-04"
    verification:
      - kind: unit
        ref: "src/server/skills.rs#parse_frontmatter_value_absent_and_parsed"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#parse_frontmatter_value_invalid_is_not_absent"
        status: pass
      - kind: other
        ref: "sed -n '/fn parse_frontmatter_value/,/^}/p' src/server/skills.rs | grep -c 'serde_yaml::' == 1, whole-file count == 1"
        status: pass
    human_judgment: false
  - id: D6
    description: "Digests are `sha256:` + 64 lowercase hex over the served bytes, produced by a per-byte `{:02x}` fold because sha2 0.11 ships no `LowerHex` impl (D-05)."
    requirement: "D-05"
    verification:
      - kind: unit
        ref: "src/server/skills.rs#sha256_digest_hex_is_prefixed_lowercase_64_hex"
        status: pass
      - kind: integration
        ref: "tests/skills_routing.rs#skills_list_returns_a_conforming_entry_on_v2 (digest asserted on the wire body)"
        status: pass
    human_judgment: false
  - id: D7
    description: "The 2.x exhaustive-enum promise holds: `pub enum ClientRequest` gains no `SkillsList`/`SkillsGet` variant, and neither method deserializes into it, while `resources/list` still does."
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#client_request_has_no_skills_variants"
        status: pass
      - kind: integration
        ref: "tests/skills_routing.rs#skills_methods_do_not_parse_as_public_client_requests"
        status: pass
    human_judgment: false
  - id: D8
    description: "Neither SEP-2640 method is name-bearing — asserted through the production `pmcp::testing::routing_name_key` seam — and a v2 POST carrying an empty `Mcp-Name` is accepted rather than refused with -32020."
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#neither_skills_method_is_routing_name_bearing"
        status: pass
      - kind: unit
        ref: "src/server/streamable_http_server.rs#skills_methods_are_not_name_bearing"
        status: pass
    human_judgment: false
  - id: D9
    description: "Transport reach is streamable HTTP only, and the recorded stdio behavior is MEASURED rather than left as prose: `parse_message` returns Err on a `skills/list` frame and Ok on a `resources/list` control (D-01)."
    requirement: "D-01"
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#stdio_ingress_rejects_a_skills_list_frame"
        status: pass
    human_judgment: false
  - id: D10
    description: "The classifier never judges a body: `skills/list` params reach the served branch RAW, including `Value::Null`, and near-miss spellings (`skills/lists`, `skills/`, `skills`) fall through to the public-enum path."
    verification:
      - kind: unit
        ref: "src/types/protocol/mod.rs#classify_internal_method_routes_skills_list_with_raw_params"
        status: pass
      - kind: unit
        ref: "src/server/streamable_http_server.rs#classify_http_ingress_routes_skills_list_with_raw_params"
        status: pass
      - kind: unit
        ref: "src/server/streamable_http_server.rs#skills_list_ingress_is_never_an_initialize"
        status: pass
    human_judgment: false

# Metrics
duration: 36 min
completed: 2026-09-02
status: complete
---

# Phase 125 Plan 01: SEP-2640 `skills/list` Tracer Summary

**`skills/list` routed end to end over streamable HTTP via the crate-private `InternalClientRequest` classifier — a live POST now returns a conforming entry with verbatim YAML frontmatter, a wire-verified `sha256:` digest and a byte-accurate size, with no new public `ClientRequest` variant.**

## Performance

- **Duration:** 36 min
- **Started:** 2026-09-02T05:13:49Z
- **Completed:** 2026-09-02T05:50:13Z
- **Tasks:** 2
- **Files modified:** 10 (9 modified, 1 created)

## Accomplishments

- **The tracer works.** A `StreamableHttpServer` built from a `Server` carrying one frontmatter-bearing skill answers a real `skills/list` POST at HTTP 200 with `result.resultType == "complete"`, a one-element `result.skills`, verbatim frontmatter including the non-required `license` field, a `resources[0].digest` matching `^sha256:[0-9a-f]{64}$` and a `size` equal to the served SKILL.md byte length. Every one of those assertions reads the WIRE body, not an in-process struct.
- **The whole architecture is proven on one path** before any expansion task builds out from it: crate-private classifier route → shared `parse_request_or_internal` seam → `HttpIngress::SkillsList` at five transport sites → thin `Server` delegate → shared `core.rs` projection → build-time entry synthesis carried as its own `Server` field.
- **`ClientRequest` gained nothing.** The 2.x exhaustive-enum promise is now guarded two independent ways — a disk-reading source scan of the enum block (with an anti-vacuity check) and a runtime `from_value` proof paired with a `resources/list` control that must parse.
- **The era split is observable, not merely written.** `ttlMs` and `cacheScope` are asserted PRESENT on the v2 response and ABSENT on the v1 twin. Those two keys have exactly one writer in the tree, so their presence is the only observable proof that the `Cacheable::Yes` named at the projection call site actually reached the envelope.
- **The D-01 stdio deferral is measured rather than asserted in prose.** `parse_message` returns `Err` on a `skills/list` frame and `Ok` on a `resources/list` control, and the test's rustdoc records the untested second link — `run_transport_actor` BREAKS its receive loop, so over stdio the frame tears the connection down rather than answering an error.
- **The frontmatter parser distinguishes three outcomes, not two**, so a broken canonical skill is diagnosable rather than indistinguishable from an unannotated one — the substrate 125-03's three distinct diagnostics need.

## Task Commits

1. **Task 1 (tracer, tdd): end-to-end `skills/list` — one skill, one path** — `227283c5` (feat)
2. **Task 2: routing guarantees — no public variant, no era gate, no routing name, no stdio reach** — `e3089151` (test)

_The tracer feedback gate ran between them: `human_verify_mode` is `end-of-phase` and the tracer's `<verify>` carries only `<automated>` blocks, so the full verify block was re-run end to end (all eight commands green) and expansion proceeded without a checkpoint._

## Files Created/Modified

- `Cargo.toml` — `serde_yaml 0.9` as an **optional** `[dependencies]` entry with a `# Consumer:` comment naming the single call site; `skills = ["dep:serde_yaml"]`. Deliberately NOT added to `full` / `full-v2` (D-09).
- `src/types/protocol/mod.rs` — `SKILLS_LIST_METHOD` / `SKILLS_GET_METHOD` single-sourced constants, `InternalClientRequest::SkillsList { params }`, the classifier arm, and two in-module tests (raw params + near-miss controls + the wire-value pin).
- `src/shared/protocol_helpers.rs` — extended the exhaustive `InternalClientRequest` match in the existing `tasks/update` routing test.
- `src/server/skills.rs` — `FrontmatterParse` (three-way), `parse_frontmatter_value` (the ONE `serde_yaml` call site), `sha256_digest_hex`, public `SkillEntry` / `SkillResourceRef`, `Skills::entries()` with its stability-boundary and disclosure rustdoc + doctest, and seven new unit tests.
- `src/server/core.rs` — `build_skills_list_response`: no era gate, `ResponseDisposition::Complete`, `Cacheable::Yes` named at the call site, single page, no `nextCursor`.
- `src/server/mod.rs` — private `Server.skill_entries: Arc<IndexMap<String, SkillEntry>>`, populated at the finalization site; `Server::handle_skills_list` as a thin delegate with no gate of its own.
- `src/server/builder.rs` — `finalize_skills_resources` now returns `(Option<Arc<dyn ResourceHandler>>, Vec<SkillEntry>)`, calling `entries()` before the consuming `into_handler()`; both `#[cfg]` call sites updated.
- `src/server/streamable_http_server.rs` — `HttpIngress::SkillsList` at all five sites, plus `assemble_skills_list_fast` / `assemble_skills_list_with_middleware` and four new in-file tests.
- `tests/skills_routing.rs` (NEW, 8 tests) — the live-wire proof and the five routing guarantees, with a numbered module-doc property table recording which tests are not redundant and why.
- `tests/v2_schema_tripwires.rs` — declared `build_skills_list_response` in the `ENVELOPE_SITES` table with its owner/cacheability decision.

## Decisions Made

See `key-decisions` in the frontmatter. The two most consequential:

1. **The frontmatter block must OPEN on line 1.** The plan said to locate the block "exactly as `parse_frontmatter_description` does", which accepts the first `---` anywhere in the leading 40 lines. Implemented stricter, and the rustdoc says why: under the looser rule a markdown horizontal rule inside an unannotated skill opens a phantom block and reports `Invalid` where `Absent` is correct — which would defeat the three-way distinction the plan added the enum for. agentskills.io requires frontmatter at the start of the file, so the stricter rule is also the conformant one.

2. **`ServerCore` gets no skills field and no handler**, exactly as the plan's "Deliberately NOT created" section requires. The `ServerCoreBuilder` call site now reads `finalize_skills_resources(...).0` with a comment recording that its dispatch accepts only the typed public `Request` enum, so a field there would be unreachable dead code.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `tests/v2_schema_tripwires.rs` `ENVELOPE_SITES` table required the new egress**

- **Found during:** Task 2 (the plan's single full `cargo test --all-features` run)
- **Issue:** `v2_schema_tripwires_every_envelope_call_site_names_its_cacheability` failed with `UNLISTED envelope call site: build_skills_list_response in src/server/core.rs at line(s) [2475]`. The tripwire exists precisely so a new `inject_v2_result_envelope` caller has to DECIDE `owner` and `cacheable` rather than inherit them by omission. The file is not in the plan's `files_modified`.
- **Fix:** Added an `EnvelopeSite` entry naming what `build_skills_list_response` decided and why — `Cacheable::Yes` because it never reaches `request_is_cacheable`, `ReservedFieldOwner::None` because it mints no reserved field, `Complete` because a listing is not a task.
- **Files modified:** `tests/v2_schema_tripwires.rs`
- **Verification:** `cargo test --all-features --test v2_schema_tripwires -- --test-threads=1` → 13 passed.
- **Committed in:** `e3089151`

**2. [Rule 3 - Blocking] `src/shared/protocol_helpers.rs` exhaustive match needed the new variant**

- **Found during:** Task 1 (mid-task compile checkpoint)
- **Issue:** `test_parse_request_or_internal_routes_tasks_update_with_raw_params` enumerates `InternalClientRequest` without a wildcard — deliberately, so a future internally-routed method cannot be silently absorbed. Adding `SkillsList` broke it. The file is not in the plan's `files_modified`.
- **Fix:** Added `SkillsList { .. }` to the panic-arm alternation, preserving the no-wildcard discipline.
- **Files modified:** `src/shared/protocol_helpers.rs`
- **Verification:** `cargo test -p pmcp --all-features --lib -- --test-threads=1` → 2152 passed.
- **Committed in:** `227283c5`

**3. [Rule 2 - Missing Critical] `parse_frontmatter_value` requires the opening delimiter on line 1**

- **Found during:** Task 1 (step 3a)
- **Issue:** Following `parse_frontmatter_description`'s "first `---` in the leading 40 lines" rule verbatim would let a markdown horizontal rule in an UNANNOTATED skill open a phantom block, reporting `Invalid` where `Absent` is correct — and, worse, occasionally reporting `Parsed` over prose. That collapses the very distinction R-04 introduced the three-way enum to preserve.
- **Fix:** The opening `---` must be the first line after an optional BOM. The rustdoc names the difference from the sibling scanner and gives this reason; a dedicated test asserts a mid-body `---` yields `Absent`.
- **Files modified:** `src/server/skills.rs`
- **Verification:** `parse_frontmatter_value_absent_and_parsed` (includes the horizontal-rule case) and `parse_frontmatter_value_invalid_is_not_absent`.
- **Committed in:** `227283c5`

**4. [Rule 3 - Blocking] Two `#[allow(dead_code)]` annotations, each with a stated reason**

- **Found during:** Task 1 (steps 2 and 7)
- **Issue:** Two plan-mandated items have no in-crate reader yet, and both tripped `dead_code` under `-D warnings`: `SKILLS_GET_METHOD` (whose route is 125-02's) and `HttpIngress::SkillsList.params` (SEP-2640's only `skills/list` param is a `cursor` and pmcp answers a single complete page).
- **Fix:** `#[allow(dead_code)]` on each, with a comment stating why the item exists ahead of its reader — for the constant, that splitting the mint across waves means a second edit to the shared rustdoc (R-06); for the field, that keeping it makes a later cursor implementation a response-assembler change rather than an ingress re-plumb. Neither is SATD.
- **Files modified:** `src/types/protocol/mod.rs`, `src/server/streamable_http_server.rs`
- **Verification:** CI-shaped `cargo clippy --all-targets --all-features -- -D warnings` → clean.
- **Committed in:** `227283c5`

**5. [Rule 2 - Missing Critical] The `FrontmatterParse::Invalid` diagnostic is READ, not just carried**

- **Found during:** Task 1 (step 3d)
- **Issue:** `Skills::entries()` skipped `Invalid` silently, so the diagnostic `String` was never read — a `dead_code` warning, and more importantly a three-way distinction that was decorative rather than load-bearing.
- **Fix:** A `tracing::debug!` on the `mcp.skills` target naming the excluded URI and the reason. Deliberately `debug!`, not `warn!`: the D-02 build-time warning that NAMES the excluded skill is 125-03's, and emitting it here would preempt that plan's semantics.
- **Files modified:** `src/server/skills.rs`
- **Verification:** `cargo clippy --all-targets --all-features -- -D warnings` → clean; `entries_excludes_frontmatter_less_and_malformed_skills` covers the path.
- **Committed in:** `227283c5`

---

**Total deviations:** 5 auto-fixed (3 × Rule 3 blocking, 2 × Rule 2 missing-critical).
**Impact on plan:** No scope creep. Two touched files outside `files_modified`, both because a shipped tripwire or an existing no-wildcard match demanded the new variant be classified — which is those guards working as designed. The one semantic departure (deviation 3) makes the parser stricter and more spec-conformant, and strengthens rather than weakens the property the plan asked for.

## Issues Encountered

**Two measurement hazards, both resolved; neither is a code defect.**

1. **`cargo test` aborts after the first failing target, so the first full-suite run never reached the tripwire.** `tests/docs04_examples_run.rs` failed with an explicit STALE-BINARY assertion (`doc_review_team` and `s50_standalone_vs_sampled` were built before `src/server/skills.rs` changed) — a harness guard, not a regression, and it told me the exact rebuild command. After `cargo build --all-features --examples` and `cargo build -p pmcp-agent -p pmcp-team-servers --all-features --examples`, the run continued and surfaced the real `v2_schema_tripwires` finding (deviation 1). **A full-suite run after an example-touching change must rebuild examples first, or its result is a lower bound on the failure count.**

2. **The `rtk` proxy compresses `cargo test` output, so a name-matching verify command reads 0 against a passing suite.** The plan's third verify command greps the reported test names; under the proxy it printed `cargo test: 3 passed` with no names and the count came back `0`. Re-run through the absolute binary (`/Users/guy/.cargo/bin/cargo`) it returns 2, which is the true value. The same proxy under-reported `git diff | wc -c` as 25,897 against a true 97,587. **Any verify command that greps command OUTPUT rather than an exit code must be run through an absolute binary path.**

## Known Stubs

Both rows are deliberate plan scope boundaries, named here so the verifier can see them rather than infer them. Neither prevents this plan's goal — a conforming single-entry `skills/list` on the wire — from being achieved.

| Stub | File / symbol | Reason and resolving plan |
|---|---|---|
| The `resources` manifest holds only the skill's own SKILL.md; reference-file rows are not emitted | `src/server/skills.rs` — `Skills::entries()` | The tracer's action explicitly scopes it: "no manifest expansion beyond the skill's own SKILL.md: those are expansion tasks." **125-03** adds reference rows alongside the 512-file / 16 MiB limits guard. The rustdoc says so at the site. |
| `Skills::entries()` returns `Result` but has no `Err` path today | `src/server/skills.rs` — `Skills::entries()` | The plan fixes the signature now so the build-time name-identity and limits validation **125-03** adds is not a source-breaking change for callers. Documented in the `# Errors` section verbatim. |
| Skills yielding `Absent` or `Invalid` are excluded with only a `debug!` breadcrumb | `src/server/skills.rs` — `Skills::entries()` | D-02's build-time WARNING naming the excluded skill, and the three distinct diagnostics built on `FrontmatterParse`, are **125-03**'s. The parser already distinguishes the cases, which is what that plan needs. |

## Threat Flags

None. Every trust boundary this plan crosses is in the plan's own `<threat_model>`, and each mitigation landed: `classify_internal_method` judges the method only and never deserializes `params` (T-125-01, asserted by the raw-params tests); `HttpIngress::SkillsList` joins the `is_initialize` `false` alternation so a skills POST can never mint a session (T-125-02, asserted by `skills_list_ingress_is_never_an_initialize`); the verbatim-frontmatter disclosure (T-125-04) and the unsigned-digest caveat (T-125-05) are both stated in rustdoc on `Skills::entries()` and `SkillResourceRef`. `serde_yaml` added zero new packages to the graph (T-125-SC).

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

**Ready for 125-02 and 125-03 (wave 2, parallel).**

- `SKILLS_GET_METHOD` is already minted and pinned to its wire value, so 125-02 adds an `InternalClientRequest::SkillsGet` arm rather than reopening the rustdoc-governed constant block. Its own test asserts today's state (`classify_internal_method(SKILLS_GET_METHOD, ..)` is `None`), so that plan flips a documented fact rather than discovering one.
- `Server.skill_entries` is an `IndexMap` keyed by SKILL.md URI, so 125-02's `skills/get` URI lookup is O(1) with no data-model change.
- The exhaustive inner `match` in `classify_http_ingress` and the no-wildcard alternations in four tests are live compile-time tripwires: adding `SkillsGet` will break the build until every site is written, which is the intended behavior.
- **Fixture rule for wave 2 (R-10):** every skill fixture authored in this phase carries frontmatter whose `name` equals the final `/` segment of its resolved URI. `tests/skills_routing.rs`'s `REFUNDS_BODY` complies and says so in its doc comment. 125-03's `validate_names` hard-reject will otherwise surface as a confusing failure inside a plan that did not author the fixture.
- **Open for 125-05:** `make quality-gate` still does not reach this module — every one of its test legs pins `--features "full"`, which excludes `skills`. This plan was verified with `cargo test --all-features`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --doc --all-features`, all green. The dedicated `make test-skills` leg (D-09) remains 125-05's.

## Self-Check: PASSED

- `tests/skills_routing.rs` exists on disk (596 lines, 8 tests, all passing).
- `git log --oneline --all | grep 227283c5` → found (Task 1, feat).
- `git log --oneline --all | grep e3089151` → found (Task 2, test).
- Plan `<verification>` re-run at close: `cargo test --all-features -- --test-threads=1` → 140 result lines, **0 FAILED**; `cargo clippy --all-targets --all-features -- -D warnings` (CI allow-list) → clean; `cargo test --doc --all-features -- --test-threads=1` → 479 passed, 0 failed; `cargo fmt --all -- --check` → clean.
- Plan `<success_criteria>`: all five met — conforming entry with verifiable digest ✅; no `ClientRequest` variant, both guards pass ✅; v1 and v2 both served while `server/discover` still answers -32601 on v1 ✅; neither method name-bearing, asserted ✅; `cargo build --all-features` and `cargo build --no-default-features` both exit 0 ✅.

---
*Phase: 125-sep-2640-conformance-skills-list-skills-get*
*Completed: 2026-09-02*
