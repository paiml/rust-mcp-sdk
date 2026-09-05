---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - Cargo.toml
  - src/types/protocol/mod.rs
  - src/server/streamable_http_server.rs
  - src/server/skills.rs
  - src/server/core.rs
  - src/server/mod.rs
  - src/server/builder.rs
  - tests/skills_routing.rs
autonomous: true
requirements: [D-01, D-04, D-05, D-07, D-11]
user_setup: []

estimate:
  tokens: 60000
  raw_tokens: 120000
  tasks: 2
  confidence: high

must_haves:
  truths:
    - "A built `Server` carrying one frontmatter-bearing skill answers a live `skills/list` POST over streamable HTTP with a `result.skills` array whose single entry carries `uri`, verbatim `frontmatter` JSON, and a `resources` manifest entry matching `^sha256:[0-9a-f]{64}$` with a `size` equal to the served byte length (D-05, ROADMAP SC#1)."
    - "`serde_json::from_value::<ClientRequest>` on a `skills/list` or `skills/get` frame still returns `Err`, while the same call on `resources/list` returns `Ok` — the 2.x exhaustive-enum promise holds (ROADMAP SC#1)."
    - "`skills/list` answers on a v1 connection as well as a v2 one — unlike `server/discover` it carries NO era gate (D-07: only the `ttlMs`/`cacheScope` attributes are 2026-07-28-conditional)."
    - "The `skills/list` result carries `resultType: \"complete\"`, contains every registered skill in one response, and emits no `nextCursor` key (D-07, D-11)."
    - "`skills/list` and `skills/get` are NOT name-bearing methods: `pmcp::testing::routing_name_key` yields nothing for them, and a v2 POST carrying an empty `Mcp-Name` is accepted."
    - "The YAML frontmatter parse is reachable through exactly one crate-private function, so the YAML crate can be swapped in a single file (D-04)."
    - "Transport reach is streamable HTTP only, and a test asserts the recorded stdio behavior rather than leaving it unmeasured (D-01)."
  artifacts:
    - Cargo.toml
    - src/types/protocol/mod.rs
    - src/server/streamable_http_server.rs
    - src/server/skills.rs
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/builder.rs
    - tests/skills_routing.rs
  key_links:
    - "`classify_internal_method` (src/types/protocol/mod.rs) -> `parse_request_or_internal` (src/shared/protocol_helpers.rs) -> `classify_http_ingress` (src/server/streamable_http_server.rs): the single interception point. A method spelling present in one and absent in the other is a silent no-route."
    - "`classify_http_ingress` inner `match` over `InternalClientRequest` is deliberately exhaustive — adding the new variants is a compile-time tripwire that forces every HttpIngress site to be written."
    - "`finalize_skills_resources` (src/server/builder.rs) -> `Server.skill_entries` (src/server/mod.rs): the ONE place both build paths get entries from. Reaching entries by downcasting the ResourceHandler breaks the day `ComposedResources` wraps it."
    - "`Skills::entries()` (public, `&self`) must be called BEFORE `Skills::into_handler()` (public, consumes `self`) — `into_handler`'s return type is public API and MUST NOT change to a tuple."
---

<objective>
Land the tracer: one frontmatter-bearing skill, registered on a `Server`, answered
end-to-end on a live `skills/list` POST over streamable HTTP with a conforming
entry whose digest is verified on the wire.

Purpose: prove the whole architecture — crate-private classifier route, entry
synthesis at build time, entries carried as their own field, shared projection in
`core.rs`, thin delegate in `mod.rs`, wire assembly in the HTTP transport — on one
path before any expansion task builds out from it. If the `InternalClientRequest`
route cannot carry `skills/list`, this is the commit that says so.

Output: a working `skills/list` over HTTP, a public `Skills::entries()` /
`SkillEntry` API, and `tests/skills_routing.rs` carrying both the live-wire proof
and the routing-guarantee proofs.
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
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-VALIDATION.md
@.claude/skills/spike-findings-rust-mcp-sdk/references/sep-2640-conformance.md
</context>

## Artifacts this phase produces

Symbols and files **created by this phase** (all five plans). Newly-created symbols
are not drift candidates — they do not exist upstream of this phase.

**Created in this plan (125-01):**

| Symbol / artifact | Kind | Location | Visibility |
|---|---|---|---|
| `SKILLS_LIST_METHOD` | const `&str` = `"skills/list"` | `src/types/protocol/mod.rs` | `pub(crate)` |
| `SKILLS_GET_METHOD` | const `&str` = `"skills/get"` | `src/types/protocol/mod.rs` | `pub(crate)` |
| `InternalClientRequest::SkillsList` | enum variant (carries raw `params`) | `src/types/protocol/mod.rs` | `pub(crate)` |
| `HttpIngress::SkillsList` | enum variant `{ id, params }` | `src/server/streamable_http_server.rs` | private |
| `SkillEntry` | `#[non_exhaustive]` struct + accessors | `src/server/skills.rs` | `pub` |
| `SkillResourceRef` | `#[non_exhaustive]` struct (`uri`/`digest`/`size`) | `src/server/skills.rs` | `pub` |
| `Skills::entries()` | `pub fn entries(&self) -> Result<Vec<SkillEntry>>` | `src/server/skills.rs` | `pub` |
| `parse_frontmatter_value` | crate-private YAML->JSON fn (the D-04 isolation point) | `src/server/skills.rs` | private |
| `sha256_digest_hex` | crate-private digest formatter | `src/server/skills.rs` | private |
| `build_skills_list_response` | shared projection free fn | `src/server/core.rs` | `pub(crate)` |
| `Server::handle_skills_list` | thin delegate | `src/server/mod.rs` | `pub(crate)` |
| `Server.skill_entries` | struct field `Arc<IndexMap<String, SkillEntry>>` | `src/server/mod.rs` | private |
| `assemble_skills_list_fast` / `assemble_skills_list_with_middleware` | HTTP response assemblers | `src/server/streamable_http_server.rs` | private |
| `tests/skills_routing.rs` | NEW integration test file | repo root `tests/` | test crate |
| `serde_yaml` optional dep + `skills = ["dep:serde_yaml"]` | Cargo feature wiring | `Cargo.toml` | — |

**Created in later plans of this phase:** `InternalClientRequest::SkillsGet`,
`HttpIngress::SkillsGet`, `build_skills_get_response`, `Server::handle_skills_get`,
`ServerCore.skill_entries` + its two delegates (125-02); the manifest-completeness,
warn+exclude, name-identity and limits validation paths in `Skills::entries()`
(125-03); `make test-skills` Makefile target, `fuzz/fuzz_targets/fuzz_skill_entry.rs`
and its `[[bin]]` registration (125-05).

**Retired in 125-04:** `SKILL_INDEX_URI`, `build_discovery_index_json`, and the
`index_json` field on `SkillsHandler`.

## Source Coverage Audit (phase-wide)

| SOURCE | ID | Feature / Requirement | Plan | Status |
|---|---|---|---|---|
| GOAL | — | A pmcp server that declares `io.modelcontextprotocol/skills` actually answers it | 01,02,03 | COVERED |
| GOAL | SC#1 | No new public `ClientRequest` variant, yet a built server answers `skills/list`+`skills/get` with verbatim frontmatter + complete `{uri,digest,size}` manifests | 01,02,03 | COVERED |
| GOAL | SC#2 | `skill://index.json` no longer served by default | 04 | COVERED |
| GOAL | SC#3 | Build-time name-identity rejection + >512-file / >16 MiB warning | 03 | COVERED |
| GOAL | SC#4 | Existing conforming behavior unchanged (byte-identical reads, refs unlisted, dual-surface byte equality, s44/c10 pass) | 03,04 | COVERED |
| GOAL | SC#5 | `resources/directory/read` and client wrappers explicitly deferred, never silently dropped | 05 | COVERED |
| REQ | — | (none — ROADMAP records `**Requirements**: TBD`; CONTEXT decisions are the tracked set) | — | N/A |
| RESEARCH | Gap #1 | `skills/list` + `skills/get` routed via the classifier | 01,02 | COVERED |
| RESEARCH | Gap #2 | Entry-manifest API (frontmatter, digest, size) | 01,03 | COVERED |
| RESEARCH | Gap #3 | `skill://index.json` retirement + 14-site blast radius | 04 | COVERED |
| RESEARCH | Gap #4 | Name-identity validation (4a unconditional, 4c frontmatter-conditional) | 03 | COVERED |
| RESEARCH | Gap #5 | 512-file / 16 MiB limits guard | 03 | COVERED |
| RESEARCH | Gap #6 | `resources/directory/read` — deferred with a record | 05 | COVERED (deferral) |
| RESEARCH | Gap #7 | Client wrappers — deferred with a record | 05 | COVERED (deferral) |
| RESEARCH | Pitfall 1 | Transport reach measured + asserted, stdio deferral recorded | 01,05 | COVERED |
| RESEARCH | Pitfall 2 | Gate blind spot — dedicated `make test-skills` leg | 05 | COVERED |
| RESEARCH | Pitfall 3 | 40+ frontmatter-less call sites keep working | 03 | COVERED |
| RESEARCH | Pitfall 4 | Frontmatter-less skill never silently synthesized | 03 | COVERED |
| RESEARCH | Pitfall 5 | `resultType` / `ttlMs` / `cacheScope`; no `request_is_cacheable` row | 01,02 | COVERED |
| RESEARCH | Pitfall 6 | Two build paths both carry entries | 01,02 | COVERED |
| RESEARCH | A1 | `sha2` 0.11 has no `LowerHex` — resolved at plan time, see 125-01 T1 | 01 | COVERED |
| RESEARCH | A2/A4 | LF + CRLF frontmatter fixtures | 01,03 | COVERED |
| RESEARCH | A3 | Contract check — resolved at plan time (no pmcp skills contract exists) | 05 | COVERED |
| CONTEXT | D-01 | HTTP-only reach; stdio a recorded deferral, not a code TODO | 01,05 | COVERED |
| CONTEXT | D-02 | Warn + exclude for frontmatter-less skills | 03 | COVERED |
| CONTEXT | D-03 | Cleanup scope: canonical surfaces only | 04 | COVERED |
| CONTEXT | D-04 | `serde_yaml` 0.9 optional, gated on `skills`, isolated behind one fn | 01 | COVERED |
| CONTEXT | D-05 | `sha2` 0.11 for `sha256:{64 lowercase hex}` | 01,03 | COVERED |
| CONTEXT | D-06 | `skills/get` unknown URI returns `-32602` | 02 | COVERED |
| CONTEXT | D-07 | `resultType: complete`; cacheability named at the projection call site | 01,02 | COVERED |
| CONTEXT | D-08 | `skill://index.json` retires | 04 | COVERED |
| CONTEXT | D-09 | Dedicated `make test-skills` leg in quality-gate | 05 | COVERED |
| CONTEXT | D-10 | Keep auto-declaring; rustdoc the HTTP-only reach + `directoryRead: false` | 05 | COVERED |
| CONTEXT | D-11 | Single page, no `nextCursor` | 01,05 | COVERED |

All rows COVERED. CONTEXT `## Deferred Ideas` (stdio reach, `resources/directory/read`,
client wrappers, strict frontmatter mode, the `resources/read` `-32601` fix) are
excluded by rule and appear in no plan as implementation work — only as recorded
deferrals in 125-05.

## Plan-time findings that supersede RESEARCH

Three facts were measured while writing this plan. They are recorded here because
they change instructions the executor would otherwise follow.

1. **RESEARCH assumption A1 is RESOLVED, and the answer is the pessimistic one.**
   `grep -rln LowerHex` over `~/.cargo/registry/src/*/sha2-0.11.0/`,
   `digest-0.11.2/` and `crypto-common-0.2.2/` returns **zero files**. There is no
   `LowerHex` impl anywhere in the shipped `sha2` 0.11 stack, so
   `format!("{:x}", hasher.finalize())` will not compile. Format the finalized
   bytes with a `{:02x}` fold. This is no longer a Wave-0 probe.

2. **`Skills::into_handler` is PUBLIC and returns `Result<Arc<dyn ResourceHandler>>`
   (`src/server/skills.rs:437`).** Changing it to return a tuple, as
   125-PATTERNS.md suggests for `finalize_skills_resources`, would be a
   semver-MAJOR break on a `pub` method. Add a separate `pub fn entries(&self)`
   taking `&self`, and have the crate-private `finalize_skills_resources` call
   `entries()` first and `into_handler()` second.

3. **The index-retirement blast radius is 2 tracked sites LARGER than RESEARCH's
   table.** `src/server/builder.rs:2302` and
   `pmcp-course/src/quizzes/ch23-skills.toml:40` both assert/describe the index and
   are absent from RESEARCH Pattern 3. Conversely `pmcp-book/book/**` and
   `pmcp-course/book/**` are **untracked mdBook output** (`git ls-files` returns
   nothing for them) and must NOT be hand-edited. Carried into 125-04.

<tasks>

<task type="tracer" tdd="true">
  <name>Task 1: End-to-end "a host asks skills/list and gets a conforming entry" — one skill, one path</name>

  <files>Cargo.toml, src/types/protocol/mod.rs, src/server/streamable_http_server.rs, src/server/skills.rs, src/server/core.rs, src/server/mod.rs, src/server/builder.rs, tests/skills_routing.rs</files>

  <read_first>
    - src/types/protocol/mod.rs — read the `InternalClientRequest` enum (~:760-816), the `SERVER_DISCOVER_METHOD` / `TASKS_UPDATE_METHOD` constant block (~:820-840), `classify_internal_method` (~:871-886), and its in-module tests (~:1066-1111). The `TasksUpdate` variant is the exact shape to copy.
    - src/shared/protocol_helpers.rs — `IngressRequest` (:15-55) and `parse_request_or_internal` (:57-100). Read the rustdoc at :32-42 stating that `classify_http_ingress` is the ONLY production consumer; that sentence is the D-01 constraint.
    - src/server/streamable_http_server.rs — the `HttpIngress` variant block ending ~:2235, `is_initialize` (~:2237-2257), `classify_http_ingress` (~:2259-2313), the v2 header-gate arm at ~:3240-3250, `InternalResponseShape` (:3747), `assemble_discover_response_fast` (:3769), `assemble_tasks_update_fast` (:3888), `assemble_tasks_update_with_middleware` (:3942), `assemble_discover_response_with_middleware` (:4923), and the four per-path dispatch arms at :5022, :5068, :5135, :5164.
    - src/server/skills.rs — the module header doctest (:1-40), `Skill` struct + accessors (:155-320), `Skills::into_handler` (:437-471), `SkillsHandler::new` (:490-512), and `parse_frontmatter_description` (:656-680) for its BOM and CRLF handling.
    - src/server/core.rs — `build_discover_response` (~:2380-2430), the `request_is_cacheable` rustdoc (~:2153-2200), and the twin-site parity note (~:2434-2440).
    - src/server/mod.rs — `Server` struct fields (:469-500), `Server::handle_discover` (~:1647-1675), `handle_tasks_update` immediately below it, and the skills finalization site (~:5365-5380).
    - src/server/builder.rs — `finalize_skills_resources` (:1426-1455) and its `#[cfg]` / `#[cfg(not)]` call-site pair (:1356-1360).
    - src/server/request_state.rs:220-230 — the in-repo `Sha256::new()` / `update` / `finalize` idiom.
    - tests/v2_tasks_update_routing.rs — the header cfg block (:53-57), the once-restated method-literal convention (:74-76), and the module-doc property table (:26-35).
    - tests/common/v2.rs — `spawn_default_config` (:385), `v2_body` (:565), `v2_headers_for` (:752), `post` (:898), `Resp` (:809). Use these; do NOT add a new spawner.
    - Cargo.toml — the `skills = []` feature line (~:306), `sha2 = "0.11"` (:149), and the `serde_yaml = "0.9"  # Consumer: ...` dev-dep line (:249) for the local dep-justification comment convention.
  </read_first>

  <behavior>
    - `classify_internal_method("skills/list", &params)` returns `Some(InternalClientRequest::SkillsList { params })` with `params` passed through undecoded, including when params is `Value::Null`.
    - `classify_internal_method("skills/lists", ...)`, `("skills/", ...)` and `("skills", ...)` all return `None`.
    - A `Skills` registry holding one skill whose body opens with a `---` YAML frontmatter block containing `name`, `description` and a non-required field yields exactly one `SkillEntry` from `Skills::entries()`, whose `frontmatter` is a JSON object carrying all three keys with the authored values, and whose `resources` manifest's first element is the skill's own SKILL.md URI with a `sha256:`-prefixed 64-lowercase-hex digest and a `size` equal to the SKILL.md body's byte length.
    - A live `StreamableHttpServer` built from a `Server` carrying that registry answers a v2 `skills/list` POST at HTTP 200 with `result.resultType == "complete"`, `result.skills` of length 1, and no `nextCursor` key anywhere in `result`.
    - The same server answers a v1-framed `skills/list` POST without `-32601` — `skills/list` carries no era gate (unlike `server/discover`).
    - `HttpIngress::SkillsList` reports `is_initialize() == false`.
  </behavior>

  <action>
Wire ONE entry point — a `skills/list` POST — through every layer to the far end of
the stack, for a single registered skill. No `skills/get`, no `ServerCore` path, no
manifest expansion beyond the skill's own SKILL.md: those are expansion tasks. Real
error handling on this one path.

1. `Cargo.toml`: add `serde_yaml = { version = "0.9", optional = true }` to
   `[dependencies]` with a `# Consumer:` comment naming `src/server/skills.rs`
   frontmatter extraction, matching the convention on the existing dev-dep line.
   Change the feature to `skills = ["dep:serde_yaml"]`. Per D-09 do NOT add
   `skills` to `full` or `full-v2` — both are enumerated lists whose drift is
   asserted by `tests/v1_severability_tripwire.rs`.

2. `src/types/protocol/mod.rs`: add `SKILLS_LIST_METHOD` and `SKILLS_GET_METHOD`
   `pub(crate) const &str` constants with the values `skills/list` and `skills/get`
   in the same block as `SERVER_DISCOVER_METHOD`, carrying the same single-sourcing
   rationale rustdoc. Before minting them, grep the crate for either literal — a
   second constant with the same value is exactly the failure that rustdoc exists
   to prevent. Add `InternalClientRequest::SkillsList { params: serde_json::Value }`
   copying the `TasksUpdate` variant's shape and its two rustdoc sections
   ("# Why it is here and NOT a `ClientRequest` variant" and the raw-params
   rationale). Add the classifier arm returning `SkillsList` with `params.clone()`.
   Do NOT add `SkillsGet` yet — 125-02 owns it. Add the in-module classifier test
   with `skills/lists` and `skills/` as near-miss controls.

3. `src/server/skills.rs`: add three items.
   (a) `parse_frontmatter_value(body: &str) -> Option<serde_json::Value>` — the ONE
   crate-private function wrapping `serde_yaml::from_str::<serde_json::Value>`, per
   D-04. It strips a leading `\u{FEFF}` BOM and locates the block between the first
   two `---` delimiter lines exactly as `parse_frontmatter_description` does, so LF
   and CRLF bodies behave identically (RESEARCH A4; `tests/skills_integration.rs:61`
   and `src/server/skills.rs:781` already lock CRLF for the description scanner).
   Returns `None` when no frontmatter block is present or the YAML does not parse to
   a JSON object.
   (b) `sha256_digest_hex(bytes: &[u8]) -> String` — `Sha256::new()` / `update` /
   `finalize`, then fold the finalized bytes into a lowercase hex string with the
   `{:02x}` width-2 formatter and prefix `sha256:`. The `{:x}` whole-value formatter
   does NOT compile on this workspace's `sha2` 0.11 stack: there is no `LowerHex`
   impl in `sha2-0.11.0`, `digest-0.11.2` or `crypto-common-0.2.2` (measured at plan
   time; supersedes RESEARCH assumption A1 and the spike's 0.10-era snippet).
   (c) `SkillEntry` and `SkillResourceRef` public structs, both `#[non_exhaustive]`
   with private fields plus accessors, deriving `Clone`, `Debug` and `Serialize`.
   `#[non_exhaustive]` is required so a later field addition stays semver-MINOR.
   `SkillEntry` carries `uri: String`, `frontmatter: serde_json::Value`, and
   `resources: Vec<SkillResourceRef>`; `SkillResourceRef` carries `uri`, `digest`,
   `size`. Serialization must emit exactly the keys `uri`, `frontmatter`,
   `resources` and `uri`, `digest`, `size`.
   (d) `pub fn entries(&self) -> Result<Vec<SkillEntry>>` on `Skills`, taking
   `&self` — NOT changing `into_handler`'s signature, which is public API returning
   `Result<Arc<dyn ResourceHandler>>` and whose return type is frozen under the 2.x
   promise. For this tracer, synthesize one entry per skill whose body yields a
   frontmatter object, with a single-element `resources` manifest holding the
   skill's own `skill_md_uri()`, the digest over `skill.body().as_bytes()`, and
   `size = skill.body().len()`. Skills with no frontmatter are skipped silently for
   now; 125-03 adds the D-02 warning and the reference-file manifest entries.
   Add rustdoc with a doctest on `entries()` (CLAUDE.md requires rustdoc + doctest
   on new public API) and a warning that `frontmatter` is emitted verbatim, so any
   author-supplied secret in a SKILL.md frontmatter block is disclosed to every
   caller (ASVS V8).

4. `src/server/builder.rs`: change the crate-private `finalize_skills_resources` to
   return `(Option<Arc<dyn ResourceHandler>>, Vec<SkillEntry>)`, calling
   `skills.entries()` BEFORE `skills.into_handler()` since the latter consumes
   `self`. Update BOTH `#[cfg]` call sites — the `skills`-enabled arm and the
   `#[cfg(not(...))]` arm, which must now produce an empty entry vector or the
   non-skills build breaks.

5. `src/server/mod.rs`: add a private `skill_entries: Arc<IndexMap<String, SkillEntry>>`
   field on `Server`, keyed by SKILL.md URI, populated at the finalization site from
   the new tuple. Carry it as its OWN field — never reach it by downcasting the
   `ResourceHandler`, which `ComposedResources` may wrap. Add
   `pub(crate) fn handle_skills_list` as a THIN delegate with zero logic that calls
   the shared `core.rs` projection, mirroring `handle_discover`, and reuse
   `handle_tasks_update`'s "It defines no gate of its own" rustdoc sentence.
   Preserve the existing cfg asymmetry: `pub mod skills` is
   `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` while the
   `ServerBuilder` skills methods are plain `#[cfg(feature = "skills")]`. Do not
   harmonize them in this phase.

6. `src/server/core.rs`: add `pub(crate) fn build_skills_list_response`, modelled on
   `build_discover_response` with four deliberate deltas. Delete the era gate — do
   not translate it; `skills/list` has no version gate in the draft. Pass
   `ResponseDisposition::Complete`, which is what emits `resultType: "complete"`
   (D-07). Name `Cacheable::Yes` at this call site, exactly as
   `build_discover_response` does, and carry the comment explaining that this is why
   `request_is_cacheable` gets no row — its `match` has no wildcard arm and its
   rustdoc calls such a row a lie about where the claim is made. Emit the result as
   `{"skills": [...]}` with all entries in one response and NO `nextCursor` key
   (D-11) — record the cursor-pagination deferral in the rustdoc, not as a code
   comment marker.

7. `src/server/streamable_http_server.rs`: add `HttpIngress::SkillsList { id, params }`
   and handle it at all five required sites. (i) Variant declaration with rustdoc
   copying the `TasksUpdate` block's reasoning. (ii) `is_initialize` — add it to the
   `false` alternation; a skills method must never mint a session (ASVS V3).
   (iii) `classify_http_ingress` — extend the fast-reject condition to also let
   `SKILLS_LIST_METHOD` through, reading the single-sourced constant and never
   re-typing the literal, then add the inner `match` arm. Omitting the fast-reject
   extension is a silent no-route bug; omitting the inner arm is a compile error.
   (iv) The v2 header-gate arm — join the request-shaped alternation, and do NOT set
   a `method_override` (that is `server/discover`-specific because its method is
   pinned by classification). (v) The TWO per-path response-assembly arms, fast path
   and middleware path, via new `assemble_skills_list_fast` and
   `assemble_skills_list_with_middleware` functions reusing the existing
   `InternalResponseShape` struct. Both must exist or the two POST paths diverge.
   Also update the in-file classification tests near the `HttpIngress::TasksUpdate`
   sites so the new variant is covered.

8. `tests/skills_routing.rs` (NEW): header
   `#![cfg(all(feature = "skills", feature = "streamable-http", feature = "http-client", not(target_arch = "wasm32")))]`
   followed by `mod common;`, mirroring `tests/v2_tasks_update_routing.rs:53-57`.
   Open with the analog's numbered module-doc property table. Restate the two method
   literals once as file-level consts with the analog's justification comment (an
   integration crate cannot reach a `pub(crate)` constant). Use
   `common::v2::spawn_default_config` with a `Server::builder()` carrying one
   frontmatter-bearing skill; do NOT add a spawner to `tests/common/v2.rs`.
   Write the live-wire test: POST a v2 `skills/list` body built with
   `v2_body` and headers from `v2_headers_for`, assert HTTP 200,
   `result.resultType == "complete"`, `result.skills` length 1, entry `uri` equals
   the registered `skill://.../SKILL.md`, `frontmatter` carries the authored `name`,
   `description` and the non-required field, `resources[0].digest` matches
   `^sha256:[0-9a-f]{64}$`, `resources[0].size` equals the SKILL.md byte length, and
   `result` has no `nextCursor` key. Add a v1-framed twin asserting the response is
   not `-32601`, since `skills/list` has no era gate.
  </action>

  <verify>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1</automated>
    <fails_when>non-zero exit, or the summary line reads "0 passed" / "running 0 tests" (a zero count here means the feature gate excluded the module, not that the code is clean)</fails_when>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests" in the output — the new integration file compiling to zero tests means its `#![cfg]` header excluded it</fails_when>
    <automated>cargo test -p pmcp --all-features --lib classify_internal_method -- --test-threads=1</automated>
    <fails_when>non-zero exit, or fewer than 2 tests reported passed (the near-miss control and the raw-params assertion must both run)</fails_when>
    <automated>cargo build --all-features 2>&amp;1 | tail -5</automated>
    <fails_when>non-zero exit, or any line containing "error[" or "error:" in the output</fails_when>
    <automated>cargo build --no-default-features 2>&amp;1 | tail -5</automated>
    <fails_when>non-zero exit — proves the `#[cfg(not(...))]` arm of the `finalize_skills_resources` tuple change was updated</fails_when>
  </verify>

  <acceptance_criteria>
    - `cargo test --all-features --test skills_routing -- --test-threads=1` exits 0 with a nonzero passed count.
    - `grep -c 'dep:serde_yaml' Cargo.toml` returns at least 1, and `grep -n '^full' Cargo.toml` shows no `skills` token on the `full` or `full-v2` lines.
    - `grep -c 'skills = \["dep:serde_yaml"\]' Cargo.toml` returns 1.
    - `cargo test --all-features --test v1_severability_tripwire -- --test-threads=1` exits 0 — the enumerated feature lists were not disturbed.
    - The live-wire test asserts a digest matching `^sha256:[0-9a-f]{64}$` on the wire response body, not on an in-process struct.
    - `serde_yaml` is named in exactly one function body in `src/server/skills.rs`: `grep -c 'serde_yaml' src/server/skills.rs` returns 1 (D-04's single-swap-point requirement).
    - `cargo build --no-default-features` exits 0.
    - `src/server/skills.rs` contains no occurrence of the whole-value lowercase-hex format specifier applied to a digest value; the digest is produced by a per-byte width-2 fold.
    - `cargo test --doc --all-features skills -- --test-threads=1` exits 0 — the new `entries()` doctest runs and passes.
  </acceptance_criteria>

  <reversibility rating="costly">
    `Skills::entries()`, `SkillEntry` and `SkillResourceRef` become public 2.x API — additive now, but removal or a field-shape change later is a MAJOR break. Mitigated by `#[non_exhaustive]` on both structs and private fields plus accessors, which keeps later field additions MINOR.
  </reversibility>

  <done>
A live `StreamableHttpServer` carrying one frontmatter-bearing skill answers a
`skills/list` POST with a single conforming entry — verbatim frontmatter, a
`sha256:` + 64-lowercase-hex digest, and a byte-accurate size — on both a v2 and a
v1 framing, and the change is committed.
  </done>
</task>

<task type="auto">
  <name>Task 2: Routing guarantees — no public variant, no era gate, no routing name</name>

  <files>tests/skills_routing.rs</files>

  <read_first>
    - tests/skills_routing.rs — the file Task 1 created; extend it, do not replace it.
    - tests/v2_tasks_update_routing.rs:1196-1208 — `client_request_has_no_tasks_update_variant`, the source-scan tripwire idiom to copy verbatim in structure.
    - src/types/protocol/mod.rs — the `pub enum ClientRequest {` declaration and the rustdoc at ~:775-790 recording that it carries no `#[non_exhaustive]`, so `enum_variant_added` is a semver-MAJOR break.
    - src/types/mrtr.rs:343-352 — `name_bearing_key`, the single table both the `Mcp-Name` emitter and the server's cross-check resolve through.
    - src/server/streamable_http_server.rs:1312 — `is_name_bearing_method`, and its literal-contract test at ~:6112.
    - contracts/mcp-protocol-sdk-v1.yaml:746-792 — the v2 header cross-check contract, including the name-bearing method table at :751.
    - tests/common/v2.rs:736-760 — `v2_headers` and `v2_headers_for`, and `pmcp::testing::routing_name_key`, the production-table seam a test must resolve through rather than restating.
  </read_first>

  <action>
Extend `tests/skills_routing.rs` with the four routing guarantees the tracer path
does not itself prove. Each is a property the phase must keep true, not a behavior
it adds.

1. The semver source-scan tripwire. Copy the structure of
   `client_request_has_no_tasks_update_variant`: read
   `src/types/protocol/mod.rs` from the repo root, locate the
   `pub enum ClientRequest {` declaration, take the block up to the first
   column-0 brace terminator, and assert the block contains neither `SkillsList`
   nor `SkillsGet`. This source scan IS the enforcement — there is no
   `cargo semver-checks` in `Makefile` or `.github/workflows/` (grep returns zero
   hits), so nothing else catches the regression.

2. The runtime wire proof, spike 008's technique. Assert
   `serde_json::from_value::<ClientRequest>` on a frame with method `skills/list`
   is `Err`, the same on `skills/get` is `Err`, and — as the load-bearing control
   that proves the assertion measures routing rather than a malformed fixture —
   the same call on a `resources/list` frame is `Ok`. A wire proof without the
   passing control proves nothing.

3. The no-era-gate property. `server/discover` answers `-32601` on a v1
   connection; `skills/list` must not. Assert a v1-framed `skills/list` POST
   against the live harness returns a `result`, and add a negative control in the
   same test asserting a v1-framed `server/discover` POST against the same server
   DOES return error code -32601 — so the assertion cannot pass vacuously against
   a server that answers everything.

4. The not-name-bearing property, and why it is a tested non-change. Neither
   method appears in `src/types/mrtr.rs`'s name-bearing table, so the v2
   `Mcp-Name` header is discarded for them. `skills/get` carries a `uri` param,
   structurally identical to `resources/read`, which IS in the table — so the
   omission looks like an oversight and must be pinned as a decision. Assert
   `pmcp::testing::routing_name_key` yields nothing for both method strings, and
   assert a v2 `skills/list` POST built with `v2_headers_for` (which derives an
   empty `Mcp-Name` through the production table) is accepted at HTTP 200 rather
   than rejected with -32020. Record in the test's rustdoc that adding either
   method to the name-bearing table is a deliberate deferral, since it would
   require editing `contracts/mcp-protocol-sdk-v1.yaml`'s method table and the
   literal-contract test in `src/server/streamable_http_server.rs`.

Extend the module-doc property table at the top of the file with a numbered row per
new test, following the analog's discipline of stating which tests are not redundant
and what control run proved it.
  </action>

  <verify>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit, or the summary reports fewer than 6 tests passed (tracer's 2 plus this task's 4)</fails_when>
    <automated>cargo test --all-features --test v2_tasks_update_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit — the analog file must be unaffected by the shared-harness usage</fails_when>
  </verify>

  <acceptance_criteria>
    - A test named for the `ClientRequest` source scan exists in `tests/skills_routing.rs` and fails if either variant name is inserted into the `pub enum ClientRequest` block (verify by asserting the test reads `src/types/protocol/mod.rs` from disk, not from a compiled constant).
    - The wire-proof test asserts `Err` on both skills method strings AND `Ok` on `resources/list` in the same test body.
    - The no-era-gate test contains both the positive `skills/list` assertion and the negative `server/discover` -32601 control.
    - The name-bearing test resolves through `pmcp::testing::routing_name_key` rather than restating a method list.
    - `cargo test --all-features --test skills_routing -- --test-threads=1` reports at least 6 passed and 0 failed.
  </acceptance_criteria>

  <done>
`tests/skills_routing.rs` fails if a `skills/*` variant is ever added to the public
`ClientRequest` enum, if `skills/list` gains an era gate, or if either method
silently becomes name-bearing. Committed.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| HTTP client -> `classify_http_ingress` | Fully attacker-controlled JSON-RPC frame: method string, id, and params. |
| `skills/list` result -> caller | Discloses the full skill catalog: every skill name, description, author-written frontmatter field, and file URI. |
| Server author's SKILL.md text -> wire | Frontmatter is emitted verbatim, so anything the author wrote in it crosses the boundary. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-125-01 | Elevation of privilege | `classify_internal_method` / `classify_http_ingress` | high | mitigate | The classifier judges the METHOD only and never deserializes `params` — a malformed body becomes a `-32602` in the served branch AFTER the header/auth pipeline, never a parse error before it. Copied verbatim from the `TasksUpdate` discipline (`src/types/protocol/mod.rs:798-806`); asserted by the raw-params classifier test. |
| T-125-02 | Spoofing / Session fixation | `HttpIngress::is_initialize` | high | mitigate | The new variant joins the `false` alternation, so a `skills/list` POST can never mint a session (ASVS V3). Site (ii) of Task 1 step 7. |
| T-125-03 | Information disclosure | `skills/list` catalog projection | medium | mitigate | Entries are projected per request from the server's own registry at the same point the resource surface is reached; `Cacheable::Yes` is named at the projection call site only, never by adding a row to `request_is_cacheable`. Cross-authorization-context caching of a filtered listing is out of scope this phase because the shipped `Skills` registry is not authorization-filtered — recorded, not silently assumed. |
| T-125-04 | Information disclosure | Verbatim `frontmatter` emission | medium | accept | The draft REQUIRES verbatim emission (SEP §Frontmatter line 241), so redaction would be non-conformant. Mitigated by rustdoc on `Skills::entries()` warning that a secret placed in SKILL.md frontmatter is disclosed to every `skills/list` caller — previously it required a `resources/read`. |
| T-125-05 | Spoofing | `sha256:` digest presented as integrity | low | accept | The draft is explicit (line 267) that digests are unsigned and supplied by the same server that supplies the content, and that hosts MUST NOT treat a digest match as a security boundary. Rustdoc must say so; pmcp must not document its digests as an integrity guarantee. |
| T-125-SC | Tampering | `serde_yaml` 0.9 addition | medium | mitigate | Package-legitimacy verdict `OK` in 125-RESEARCH.md `## Package Legitimacy Audit`: crates.io since 2016-02-27, 6.8M weekly downloads, repo `github.com/dtolnay/serde-yaml`, already resolved in `Cargo.lock` at `0.9.34+deprecated` and already a production dep of four workspace crates — zero new packages enter the graph. `cargo audit` measured exit 0 with neither `serde_yaml` nor `unsafe-libyaml` among the 7 allowed warnings. No `[ASSUMED]` or `[SUS]` package is installed by this plan, so no blocking human checkpoint is required. |
</threat_model>

<verification>
- `cargo test --all-features -- --test-threads=1` (matches CI `.github/workflows/ci.yml:104`) exits 0.
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0 — `make lint` pins `--features "full"` and does not reach this module.
- `cargo test --doc --all-features -- --test-threads=1` exits 0 — `make test-doc` also pins `--features "full"`.
- Do NOT accept `make quality-gate` alone as this plan's verification: measured in 125-RESEARCH.md Pitfall 2, every one of its test legs pins `--features "full"`, which excludes `skills`, so a green gate here proves nothing about this code until 125-05 lands `make test-skills`.
</verification>

<success_criteria>
- A `skills/list` POST over streamable HTTP returns a conforming single entry with a verifiable digest.
- `ClientRequest` gains no variant; the source-scan tripwire and the runtime wire proof both pass.
- `skills/list` answers on v1 and v2 alike; `server/discover` still answers -32601 on v1.
- Neither method is name-bearing, and that is asserted rather than assumed.
- `cargo build --no-default-features` and `cargo build --all-features` both exit 0.
</success_criteria>

<output>
Create `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md` when done.
</output>
